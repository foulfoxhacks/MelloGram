//! Membership and relationship events: bans, kicks, restrictions and lost
//! access, detected from Telegram participant updates, service messages and
//! chat-list changes. Classification is pure so it can be tested without a
//! network; the Telegram backend maps raw updates onto [`ParticipantState`].

use serde::{Deserialize, Serialize};

/// A participant's standing in a chat, reduced to what matters for alerts.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ParticipantState {
    /// Not in the chat (or unknown).
    Absent,
    Member,
    Admin,
    Creator,
    /// Left on their own.
    Left,
    /// Banned (`view_messages` = cannot see the chat) or restricted.
    Banned { view_messages: bool, until: i64 },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum MembershipKind {
    /// Banned: removed and blocked from rejoining.
    Banned,
    /// Removed by someone else without a lasting ban.
    Removed,
    /// Can still read, but some rights were taken away.
    Restricted,
    /// A ban or restriction was lifted.
    Unbanned,
    Left,
    Joined,
    Promoted,
    Demoted,
    /// The chat disappeared from your list or became inaccessible.
    AccessLost,
}

impl MembershipKind {
    pub fn is_ban_like(self) -> bool {
        matches!(self, Self::Banned | Self::Removed | Self::Restricted | Self::AccessLost)
    }
}

/// Classifies a participant change. `actor_is_user` is true when the person
/// changed their own state (for example leaving), which is never a ban.
pub fn classify(prev: ParticipantState, new: ParticipantState, actor_is_user: bool) -> Option<MembershipKind> {
    use ParticipantState as P;
    let was_in = matches!(prev, P::Member | P::Admin | P::Creator | P::Banned { view_messages: false, .. });
    match (prev, new) {
        (P::Banned { .. }, P::Absent | P::Member | P::Left) => Some(MembershipKind::Unbanned),
        (_, P::Banned { view_messages: true, .. }) => Some(MembershipKind::Banned),
        (P::Banned { view_messages: false, .. }, P::Banned { view_messages: false, .. }) => None,
        (_, P::Banned { view_messages: false, .. }) => Some(MembershipKind::Restricted),
        (_, P::Left | P::Absent) if was_in => Some(if actor_is_user { MembershipKind::Left } else { MembershipKind::Removed }),
        (P::Absent | P::Left, P::Member | P::Admin | P::Creator) => Some(MembershipKind::Joined),
        (P::Member, P::Admin | P::Creator) => Some(MembershipKind::Promoted),
        (P::Admin | P::Creator, P::Member) => Some(MembershipKind::Demoted),
        _ => None,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum EventSource {
    ParticipantUpdate,
    ServiceMessage,
    ChatList,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MembershipEvent {
    pub chat_id: i64,
    pub chat_title: Option<String>,
    pub user_id: i64,
    pub user_name: Option<String>,
    pub actor_id: Option<i64>,
    pub actor_name: Option<String>,
    pub kind: MembershipKind,
    /// Unix seconds when a ban or restriction ends; `None` = permanent.
    pub until: Option<i64>,
    /// The event is about you.
    pub is_self: bool,
    /// Unix seconds.
    pub date: i64,
    pub source: EventSource,
}

impl MembershipEvent {
    /// One-line human description, e.g. "You were banned from Mello Devs by Ava".
    pub fn describe(&self) -> String {
        let chat = self.chat_title.as_deref().unwrap_or("a chat");
        let who = if self.is_self { "You".to_owned() } else { self.user_name.clone().unwrap_or_else(|| format!("User {}", self.user_id)) };
        let (were, was) = if self.is_self { ("were", "were") } else { ("was", "was") };
        let by = match (&self.actor_name, self.actor_id) {
            (Some(n), _) => format!(" by {n}"),
            (None, Some(id)) if id != self.user_id && id != 0 => format!(" by user {id}"),
            _ => String::new(),
        };
        let until = match self.until {
            Some(t) if t > 0 => format!(" until {}", chrono::DateTime::from_timestamp(t, 0).map(|d| d.format("%Y-%m-%d %H:%M UTC").to_string()).unwrap_or_default()),
            _ => String::new(),
        };
        match self.kind {
            MembershipKind::Banned => format!("{who} {were} banned from {chat}{by}{until}"),
            MembershipKind::Removed => format!("{who} {were} removed from {chat}{by}"),
            MembershipKind::Restricted => format!("{who} {were} restricted in {chat}{by}{until}"),
            MembershipKind::Unbanned => format!("{who} {was} unbanned in {chat}{by}"),
            MembershipKind::Left => format!("{who} left {chat}"),
            MembershipKind::Joined => format!("{who} joined {chat}"),
            MembershipKind::Promoted => format!("{who} {was} made an admin in {chat}{by}"),
            MembershipKind::Demoted => format!("{who} {was} removed as admin in {chat}{by}"),
            MembershipKind::AccessLost => format!("{who} lost access to {chat} (removed, banned, or the chat was deleted)"),
        }
    }
}

/// Chats present in `before` but missing from `after`. Only meaningful when
/// `after` is the complete chat list; otherwise returns nothing so chats that
/// merely scrolled out of a paginated list are never reported.
pub fn lost_chats(before: &[(i64, String)], after: &[(i64, String)], after_complete: bool) -> Vec<(i64, String)> {
    if !after_complete || before.is_empty() {
        return Vec::new();
    }
    before.iter().filter(|(id, _)| !after.iter().any(|(a, _)| a == id)).cloned().collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use ParticipantState as P;

    #[test]
    fn classifies_bans_kicks_restrictions() {
        assert_eq!(classify(P::Member, P::Banned { view_messages: true, until: 0 }, false), Some(MembershipKind::Banned));
        assert_eq!(classify(P::Member, P::Banned { view_messages: false, until: 99 }, false), Some(MembershipKind::Restricted));
        assert_eq!(classify(P::Member, P::Absent, false), Some(MembershipKind::Removed));
        assert_eq!(classify(P::Member, P::Left, true), Some(MembershipKind::Left));
        assert_eq!(classify(P::Banned { view_messages: true, until: 0 }, P::Absent, false), Some(MembershipKind::Unbanned));
        assert_eq!(classify(P::Absent, P::Member, true), Some(MembershipKind::Joined));
        assert_eq!(classify(P::Member, P::Admin, false), Some(MembershipKind::Promoted));
        assert_eq!(classify(P::Admin, P::Member, false), Some(MembershipKind::Demoted));
        assert_eq!(classify(P::Member, P::Member, false), None);
        assert_eq!(classify(P::Banned { view_messages: false, until: 1 }, P::Banned { view_messages: false, until: 2 }, false), None);
        // Leaving yourself is never reported as a ban.
        assert_eq!(classify(P::Admin, P::Absent, true), Some(MembershipKind::Left));
    }

    #[test]
    fn describes_events() {
        let mut e = MembershipEvent {
            chat_id: -100,
            chat_title: Some("Mello Devs".into()),
            user_id: 1,
            user_name: Some("Kai".into()),
            actor_id: Some(2),
            actor_name: Some("Ava".into()),
            kind: MembershipKind::Banned,
            until: None,
            is_self: true,
            date: 0,
            source: EventSource::ParticipantUpdate,
        };
        assert_eq!(e.describe(), "You were banned from Mello Devs by Ava");
        e.is_self = false;
        e.kind = MembershipKind::Restricted;
        e.until = Some(1_800_000_000);
        assert_eq!(e.describe(), "Kai was restricted in Mello Devs by Ava until 2027-01-15 08:00 UTC");
        e.kind = MembershipKind::AccessLost;
        e.is_self = true;
        assert!(e.describe().starts_with("You lost access to Mello Devs"));
    }

    #[test]
    fn lost_chats_only_with_complete_list() {
        let before = vec![(1, "A".to_owned()), (2, "B".to_owned()), (3, "C".to_owned())];
        let after = vec![(1, "A".to_owned()), (3, "C".to_owned())];
        assert_eq!(lost_chats(&before, &after, true), vec![(2, "B".to_owned())]);
        assert!(lost_chats(&before, &after, false).is_empty());
        assert!(lost_chats(&[], &after, true).is_empty());
    }
}

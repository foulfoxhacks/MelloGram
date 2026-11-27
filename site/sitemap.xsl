<?xml version="1.0" encoding="UTF-8"?>
<xsl:stylesheet version="1.0"
  xmlns:xsl="http://www.w3.org/1999/XSL/Transform"
  xmlns:s="http://www.sitemaps.org/schemas/sitemap/0.9"
  xmlns:image="http://www.google.com/schemas/sitemap-image/1.1"
  exclude-result-prefixes="s image">
  <xsl:output method="html" encoding="UTF-8" indent="yes"/>
  <xsl:template match="/">
    <html lang="en">
      <head>
        <meta charset="UTF-8"/>
        <meta name="viewport" content="width=device-width, initial-scale=1"/>
        <title>MelloGram XML Sitemap</title>
        <meta name="description" content="The official MelloGram website XML sitemap, published by FoulFoxHacks."/>
        <style>
          :root{color-scheme:dark;--bg:#11121b;--panel:#1b1d2a;--line:#303349;--text:#f3f2ff;--muted:#aaaec5;--purple:#a995ff;--blue:#72b8ff}
          *{box-sizing:border-box}body{margin:0;background:radial-gradient(ellipse at 50% -15%,#393064 0,transparent 48%),var(--bg);color:var(--text);font:15px/1.6 system-ui,-apple-system,"Segoe UI",sans-serif}
          .wrap{width:min(1040px,calc(100% - 32px));margin:48px auto}.hero{display:flex;align-items:center;gap:20px;padding:26px 28px;border:1px solid var(--line);border-radius:22px;background:linear-gradient(135deg,#242139,#1b2234);box-shadow:0 24px 80px #0005}
          .logo{width:76px;height:76px;object-fit:contain;flex:none}.eyebrow{margin:0 0 4px;color:var(--purple);font-size:12px;font-weight:800;letter-spacing:.13em;text-transform:uppercase}
          h1{margin:0;font-size:clamp(27px,5vw,42px);line-height:1.1;letter-spacing:-.04em}.sub{margin:8px 0 0;color:var(--muted)}
          .stats{display:flex;flex-wrap:wrap;gap:10px;margin:22px 0}.pill{padding:7px 12px;border:1px solid var(--line);border-radius:999px;background:#1a1b28;color:var(--muted);font-size:13px}
          .panel{overflow:hidden;border:1px solid var(--line);border-radius:18px;background:var(--panel)}.panel-head{padding:17px 20px;border-bottom:1px solid var(--line);font-weight:750}
          table{width:100%;border-collapse:collapse}th,td{text-align:left;padding:13px 20px;border-bottom:1px solid var(--line);vertical-align:top}th{color:var(--muted);font-size:12px;letter-spacing:.08em;text-transform:uppercase}tr:last-child td{border-bottom:0}
          a{color:var(--blue);text-decoration:none;overflow-wrap:anywhere}a:hover{color:var(--purple);text-decoration:underline}.loc{font-weight:650}.muted{color:var(--muted);font-size:13px}
          footer{padding:22px 4px;color:var(--muted);font-size:13px}footer a{color:var(--purple)}
          @media(max-width:650px){.wrap{margin:20px auto}.hero{padding:20px;align-items:flex-start}.logo{width:54px;height:54px}th,td{padding:11px 12px}.lastmod{display:none}}
        </style>
      </head>
      <body>
        <main class="wrap">
          <header class="hero">
            <img class="logo" src="/assets/logo.png" alt="MelloGram brand logo"/>
            <div><p class="eyebrow">FoulFoxHacks · Official project index</p><h1>MelloGram Sitemap</h1>
              <p class="sub">Discover the official product pages, downloads, documentation and resources.</p></div>
          </header>
          <div class="stats">
            <span class="pill">Website: <a href="https://mellogram.foulfoxhacks.dev/">mellogram.foulfoxhacks.dev</a></span>
            <span class="pill">Pages indexed: <strong><xsl:value-of select="count(s:urlset/s:url)"/></strong></span>
            <span class="pill">Format: XML Sitemap Protocol</span>
          </div>
          <section class="panel">
            <div class="panel-head">Canonical pages</div>
            <table>
              <thead><tr><th>Page URL</th><th class="lastmod">Last modified</th><th>Priority</th></tr></thead>
              <tbody>
                <xsl:for-each select="s:urlset/s:url">
                  <tr>
                    <td><a class="loc" href="{s:loc}"><xsl:value-of select="s:loc"/></a>
                      <xsl:if test="image:image"><div class="muted"><xsl:value-of select="count(image:image)"/> indexed image(s)</div></xsl:if>
                    </td>
                    <td class="lastmod"><xsl:value-of select="s:lastmod"/></td>
                    <td><xsl:value-of select="s:priority"/></td>
                  </tr>
                </xsl:for-each>
              </tbody>
            </table>
          </section>
          <footer>Published by <a href="https://github.com/foulfoxhacks">FoulFoxHacks</a> for MelloGram. This presentation layer does not change the underlying machine-readable XML sitemap.</footer>
        </main>
      </body>
    </html>
  </xsl:template>
</xsl:stylesheet>

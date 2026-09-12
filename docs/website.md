# Public website

The user-facing website lives in `pages/`. It uses static HTML and CSS with local
assets, no build dependencies, no analytics, and no third-party fonts. The desktop
application and its build remain separate.

## Preview

Run `python3 -m http.server 8080 --directory pages` from the repository root and
open `http://localhost:8080`. Check both the landing page and privacy page at
mobile and desktop widths. Relative asset and navigation URLs support both a
GitHub project subpath and a custom domain.

## Publish

In repository Settings > Pages, select **GitHub Actions** as the source. The
Website workflow publishes only `pages/` when changes to that directory or the
workflow reach `main`. It does not depend on release creation or application
packaging. Manual dispatch is supported on `main` only. Configure the
`github-pages` environment to allow deployments from `main`.

Changes follow the normal feature branch, approved PR, and Gitflow process.
No deployment runs from feature branches or `develop`.

## Content maintenance

Keep feature descriptions aligned with README.md and the user wiki. The website
privacy page reproduces PRIVACY.md and adds a separate website hosting notice.
When the application policy changes, update both copies together, preserving
its effective date. The policy currently names Windows and macOS audio outputs;
review the source policy for Ubuntu coverage before changing the legal text.
Keep `pages/license.txt` identical to LICENSE. Do not add analytics, remote fonts,
or third-party embeds without reviewing the website privacy notice.

## Permanent installer links

Download buttons use GitHub's `releases/latest/download/` URLs. Release CI adds
fixed-name copies of the macOS ZIP, Windows installer, and Ubuntu DEB while
retaining versioned assets. `scripts/prepare-release-downloads.sh` validates all
three installers before copying them. Prereleases also receive these assets,
but the latest stable URLs do not select prereleases.

The website does not need to be rebuilt when a release is published. Before
first deploying these links, publish a stable release with the updated workflow
or upload the matching fixed-name copies to the existing latest stable release.
Older releases do not gain these files automatically.

## Search and assistant discovery

The canonical origin is `https://svb.miguel.ms`. Both HTML pages declare canonical,
Open Graph, social-card, and JSON-LD metadata. The sitemap lists only canonical
HTML pages. robots.txt allows crawling and links to the sitemap. llms.txt is a
curated project guide, an emerging convention rather than a ranking guarantee.
Keep its claims consistent with the visible page and privacy policy. No ratings,
reviews, or unverified compatibility claims are added to structured data.

After deployment, submit the sitemap in Google Search Console and Bing Webmaster
Tools. Validate structured data with their inspection tools. Updating these files
does not itself submit the site or guarantee indexing or rich results.

## Markdown versions

Every HTML page has a generated Markdown counterpart (`index.md`, `privacy.md`).
The HTML alternate link and llms.txt point to these files. Run
`python3 scripts/generate-page-markdown.py` after editing page content, and
`python3 scripts/generate-page-markdown.py --check` to detect stale copies.
The conversion includes main content and footer notices, preserves absolute
links, and omits navigation and decorative graphics. The sitemap continues to
list the canonical HTML pages only.

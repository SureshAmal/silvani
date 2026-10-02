# Anime Streaming Providers: Video Extraction Mechanics

Detailed breakdown of how major anime streaming sites work, their embed video hosts, their APIs, and how to extract video streams from them.

---

## 1. HiAnime / AniWatch (`hianime.at`, `aniwatch.lu`)
* **Underlying Video Hosts:** ZokoAnime, MegaCloud, Vidstream, RapidCloud.
* **Extraction Flow:**
  1. `GET /api/theme/episode/servers?episodeId={ep_id}` -> Returns `data-hash`.
  2. Base64 decode `data-hash` -> Gives embed URL (`https://zokoanime.video/stream/...`).
  3. `GET {embed_url}` (Header: `Referer: https://zokoanime.video/`) -> Contains `window.__P="{blob}"`.
  4. Base64 decode blob and XOR with byte key `otaku-embed-v1`.
* **Can We Extract Video Directly?** **YES.** Returns direct master `.m3u8` with 1080p, 720p, 360p, and soft VTT subtitles.

---

## 2. GogoAnime / AniTaku (`anitaku.pe`, `gogoanime3.co`)
* **Underlying Video Hosts:** Vidstreaming, GogoCDN, Streamtape, Filemoon.
* **Extraction Flow:**
  1. Scrape episode page for iframe `src` (`https://emturkey.pro/streaming.php?id={ID}`).
  2. Decrypt `id` query parameter using AES-256-CBC with community secret key.
  3. `GET /encrypt-ajax.php?id={encrypted_data}` (Header: `X-Requested-With: XMLHttpRequest`).
  4. Response is AES encrypted; decrypt using second key to get JSON.
* **Can We Extract Video Directly?** **YES.** Returns HLS `.m3u8` stream and fallback direct MP4 URLs.

---

## 3. AnimePahe (`animepahe.ru`, `animepahe.pw`)
* **Underlying Video Hosts:** Kwik (`kwik.cx`).
* **Extraction Flow:**
  1. `GET /api?m=links&id={session_id}` -> Returns list of Kwik URLs for resolutions.
  2. `GET https://kwik.cx/e/{id}` (Header: `Referer: https://animepahe.ru/`).
  3. Parse HTML script tag containing Dean Edwards packed JS: `eval(function(p,a,c,k,e,d)...)`.
  4. Unpack JS string to find the HLS source variable.
* **Can We Extract Video Directly?** **YES.** Yields direct `.m3u8` (720p/1080p high compression).

---

## 4. AllAnime / MKissa (`allanime.day`, `allmanga.to`)
* **Underlying Video Hosts:** Wixmp, DropLoad, Swish, GDrivePlayer.
* **Extraction Flow:**
  1. `GET https://api.allanime.day/api` passing GraphQL payload with `showId` and `episodeString`.
  2. Response contains array of `sourceUrl` strings.
  3. Hex-decode strings (or decrypt with simple cipher) -> Reveals CDN URL.
* **Can We Extract Video Directly?** **YES.** Gives `.m3u8` playlists directly from CDN.

---

## 5. KickAssAnime (`kaa.lt`, `kaas.to`)
* **Underlying Video Hosts:** DuckStream, BirdStream, StreamWish.
* **Extraction Flow:**
  1. `POST /api/show/{slug}/episode/{num}` -> Returns server list JSON.
  2. Embed endpoint returns AES-encrypted payload.
  3. Decrypt token using client key -> Yields master HLS playlist and subtitle array.
* **Can We Extract Video Directly?** **YES.** Provides HLS streams with multi-language soft subtitles.

---

## 6. WCOStream / WcoForever (`wcostream.tv`, `wcofun.net`)
* **Underlying Video Hosts:** cdn.wco.tv, Internal CDN.
* **Extraction Flow:**
  1. Scrape episode page for player iframe.
  2. In iframe source, extract Base64 encoded JavaScript array or JSON server token.
  3. Decodes to a direct MP4 file URL hosted on WCO CDN servers.
* **Can We Extract Video Directly?** **YES.** Provides raw MP4 video URLs directly.

---

## 7. Hindi Dubbed Sites (AnimeWorld India, WatchAnimeWorld, AnimeSalt)
* **Underlying Video Hosts:** StreamWish, FileLions, DoodStream, Mega, GDrive.
* **Extraction Flow:**
  * **StreamWish (`streamwish.to` / `swish.com`):** Scrape embed page, unpack JS `eval(p,a,c,k,e,d)`, extract `.m3u8` link from `sources: [{file: "..."}]`.
  * **FileLions (`filelions.to`):** Same JS unpacker mechanics -> Returns master `.m3u8`.
  * **DoodStream (`dood.to`):** Extract pass URL token from `/pass_md5/...`, query with referer -> Returns direct MP4 link.
* **Can We Extract Video Directly?** **YES.** Handled via universal host scrapers (or `yt-dlp`).

---

## 8. Summary Comparison Table

| Provider | Video Format | Main Host | Decryption Needed | Direct Extraction Feasible? |
| :--- | :--- | :--- | :--- | :--- |
| **HiAnime / AniWatch** | `.m3u8` (HLS) | ZokoAnime / MegaCloud | Base64 + XOR (`otaku-embed-v1`) | **Yes (Easiest & Fastest)** |
| **GogoAnime** | `.m3u8` & `.mp4` | Vidstreaming / GogoCDN | AES-256-CBC | **Yes (Standard)** |
| **AnimePahe** | `.m3u8` (HLS) | Kwik | JS Unpacker (`p,a,c,k,e,d`) | **Yes (Lightweight)** |
| **AllAnime** | `.m3u8` (HLS) | Wixmp / Swish | Hex decoding | **Yes (GraphQL API)** |
| **KickAssAnime** | `.m3u8` (HLS) | DuckStream | AES API token | **Yes (Multi-lang)** |
| **WCOStream** | `.mp4` | WCO Internal CDN | Base64 decode | **Yes (Direct file)** |
| **Hindi Sites** | `.m3u8` / `.mp4` | StreamWish / FileLions | JS Unpacker | **Yes (Universal extractor)** |

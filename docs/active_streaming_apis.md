# Active Anime Streaming APIs: Provider-by-Provider Specifications

This document defines the exact HTTP request specifications, inputs, headers, deobfuscation algorithms, and outputs for all verified active anime streaming providers (English Sub, English Dub, and Hindi Dub).

---

## Table of Contents
1. [HiAnime / AniWatch (`hianime.at`)](#1-hianime--aniwatch-hianimeat)
2. [GogoAnime / AniTaku (`anitaku.pe`)](#2-gogoanime--anitaku-anitakupe)
3. [KickAssAnime (`kaa.lt`)](#3-kickassanime-kaalt)
4. [AnimeOnsen (`animeonsen.xyz`)](#4-animeonsen-animeonsenxyz)
5. [AnimeSuge / Anikoto (`anikototv.to`)](#5-animesuge--anikoto-anikototto)
6. [YugenAnime (`yugenanime.tv`)](#6-yugenanime-yugenanimetv)
7. [AllAnime (`allanime.day`)](#7-allanime-allanimeday)
8. [OnePace (`onepace.net`)](#8-onepace-onepacenet)
9. [AnimeWorld India (`animeworldindia.net`)](#9-animeworld-india-animeworldindianet)
10. [Toonworld4all (`toonworld4all.me`)](#10-toonworld4all-toonworld4allme)
11. [DesiDubAnime (`desidubanime.me`)](#11-desidubanime-desidubanimeme)
12. [AnimeTM (`animetm.net`)](#12-animetm-animetmnet)
13. [Official Channels: Muse India & Ani-One India](#13-official-channels-muse-india--ani-one-india)

---

## 1. HiAnime / AniWatch (`hianime.at`)

### What It Expects
* **Search:** String query (spaces replaced with `+`).
* **Episode List:** Numeric ID suffix of the anime slug.
* **Server Resolution:** Numeric episode ID.

### API Endpoints & Request Flow
1. **Search Query:**
   * `GET https://hianime.at/search?keyword={query}`
   * Headers: `User-Agent: Mozilla/5.0 ...`
   * Output: HTML markup containing `.film-name a` with slug (e.g. `one-piece-1`).
2. **Episode List:**
   * `GET https://hianime.at/api/theme/episode/list/{numeric_id}` (e.g., `1` from `one-piece-1`)
   * Output: JSON containing HTML with `data-id="{ep_id}"` and `data-number="{ep_num}"`.
3. **Server Hash:**
   * `GET https://hianime.at/api/theme/episode/servers?episodeId={ep_id}`
   * Output: JSON containing HTML with `data-hash="{base64_hash}"` under server `ZokoAnime` or `MegaCloud`.
4. **Embed Page:**
   * `GET {base64_decoded_url}` (e.g., `https://zokoanime.video/stream/mal/{mal_id}/{ep_num}/sub`)
   * Headers: `Referer: https://zokoanime.video/`
   * Output: HTML with inline script `window.__P="{blob}"`.

### Deobfuscation Algorithm
* Base64 decode `{blob}` into byte array.
* XOR each byte with repeating ASCII key `"otaku-embed-v1"` (byte array: `[111, 116, 97, 107, 117, 45, 101, 109, 98, 101, 100, 45, 118, 49]`).
* Parse resulting string as JSON.

### Output
```json
{
  "src": "https://hls.dramahot.top/v/.../master.m3u8",
  "subtitles": [
    { "src": "https://.../sub.vtt", "label": "English", "default": true }
  ]
}
```
* **Playback Header Required:** `Referer: https://zokoanime.video/`

---

## 2. GogoAnime / AniTaku (`anitaku.pe`)

### What It Expects
* **Search:** Anime title.
* **Episode Page:** Slug and episode number.
* **Crypto Keys:** AES-256 keys (community maintained) for query encryption and response decryption.

### API Endpoints & Request Flow
1. **Search Query:**
   * `GET https://anitaku.pe/search.html?keyword={query}`
   * Output: HTML with anime slugs (Note: English dubbed series are separate slugs ending in `-dub`).
2. **Episode Page Iframe:**
   * `GET https://anitaku.pe/{slug}-episode-{num}`
   * Output: HTML containing video iframe:
     `<iframe src="https://emturkey.pro/streaming.php?id={ENCRYPTED_ID}&title=...">`
3. **AJAX Stream Request:**
   * `GET https://emturkey.pro/encrypt-ajax.php?id={AES_ENCRYPTED_PARAMS}&alias={ID}`
   * Headers:
     ```http
     X-Requested-With: XMLHttpRequest
     Referer: https://emturkey.pro/streaming.php?id=...
     ```
   * Output: AES-encrypted JSON string.

### Deobfuscation Algorithm
* AES-256-CBC decrypt the response payload using community decryption key and IV.
* Resulting JSON contains an array of `source` objects.

### Output
```json
{
  "source": [
    { "file": "https://.../master.m3u8", "label": "auto", "type": "hls" },
    { "file": "https://.../1080.mp4", "label": "1080P", "type": "mp4" }
  ],
  "source_bk": [...]
}
```
* **Playback Header Required:** `Referer: https://anitaku.pe/`

---

## 3. KickAssAnime (`kaa.lt`)

### What It Expects
* **Search:** Anime query string.
* **API Route:** JSON API requests with session cookies.

### API Endpoints & Request Flow
1. **Search Query:**
   * `GET https://kaa.lt/api/show/search?q={query}`
   * Output: JSON array of show objects with title and `slug`.
2. **Episode List:**
   * `GET https://kaa.lt/api/show/{slug}/episodes`
   * Output: JSON array containing episode numbers and episode `slug` or `id`.
3. **Server & Video Sources:**
   * `GET https://kaa.lt/api/show/{slug}/episode/{num}`
   * Output: JSON containing server list (DuckStream, BirdStream) with encrypted source IDs.
4. **Player Stream Endpoint:**
   * `POST https://kaa.lt/api/show/{slug}/episode/{num}/{server_name}`
   * Output: Encrypted HLS master link payload.

### Output
* Returns `.m3u8` master playlist with multi-audio tracks and soft VTT subtitles.
* **Playback Header Required:** `Referer: https://kaa.lt/`

---

## 4. AnimeOnsen (`animeonsen.xyz`)

### What It Expects
* **Direct REST API queries** (no HTML scraping or browser simulation required).

### API Endpoints & Request Flow
1. **Search Anime:**
   * `GET https://api.animeonsen.xyz/v4/search/{query}`
   * Output: JSON array of anime metadata items with unique anime `id`.
2. **Episode List:**
   * `GET https://api.animeonsen.xyz/v4/anime/{id}/episodes`
   * Output: JSON array of episode IDs and numbers.
3. **Stream Video Link:**
   * `GET https://api.animeonsen.xyz/v4/episode/{episode_id}/video`
   * Headers: `User-Agent: Mozilla/5.0 ...`
   * Output: JSON direct video URL response.

### Output
```json
{
  "uri": "https://stream.animeonsen.xyz/content/.../video.mp4",
  "subtitles": [
    { "lang": "en", "src": "https://.../en.vtt" }
  ]
}
```
* **Playback Header Required:** Standard `User-Agent`. Direct playback supported.

---

## 5. AnimeSuge / Anikoto (`anikototv.to`)

### What It Expects
* **Anime Slug:** Path identifier from search.
* **Episode Query:** Numeric episode index.

### API Endpoints & Request Flow
1. **Search:**
   * `GET https://anikototv.to/filter?keyword={query}`
   * Output: HTML with anime cards and slugs.
2. **Episode Map:**
   * `GET https://anikototv.to/ajax/episode/list/{anime_id}`
   * Output: HTML with episode buttons and `data-id`.
3. **Server Embed:**
   * `GET https://anikototv.to/ajax/episode/servers?episodeId={data_id}`
   * Output: JSON with embed iframe source URL (RapidStream / MegaCloud).
4. **Video Resolution:**
   * Scrapes iframe container and extracts `.m3u8` stream playlist.

### Output
* HLS master playlist URL (`.m3u8`) with adaptive bitrates.
* **Playback Header Required:** `Referer: https://anikototv.to/`

---

## 6. YugenAnime (`yugenanime.tv`)

### What It Expects
* **Anime Slug:** Series slug from search.
* **CSRF Token:** Parsed from episode page for POST requests.

### API Endpoints & Request Flow
1. **Search:**
   * `GET https://yugenanime.tv/discover/?q={query}`
   * Output: HTML search results with series URLs (e.g. `/anime/123/one-piece/`).
2. **Watch Page:**
   * `GET https://yugenanime.tv/watch/{id}/{slug}/{ep_num}/`
   * Output: HTML page containing embedded player iframe and CSRF token.
3. **Embed API Call:**
   * `POST https://yugenanime.tv/api/embed/`
   * Headers:
     ```http
     X-Requested-With: XMLHttpRequest
     X-CSRFToken: {csrf_token}
     Referer: https://yugenanime.tv/watch/...
     ```
   * Body: Form data with episode and server IDs.
   * Output: JSON response with video player configuration.

### Output
```json
{
  "hls": ["https://.../master.m3u8"],
  "sources": [...]
}
```
* **Playback Header Required:** `Referer: https://yugenanime.tv/`

---

## 7. AllAnime (`allanime.day`)

### What It Expects
* **GraphQL Query String:** Sent as URL-encoded parameter to `/api`.

### API Endpoints & Request Flow
1. **GraphQL Search:**
   * `GET https://api.allanime.day/api?variables={"search":{"query":"{query}","allowAdult":false}}&extensions={"persistedQuery":{"sha256Hash":"..."}}`
   * Output: JSON array of show objects with `_id` and titles.
2. **Episode Query:**
   * `GET https://api.allanime.day/api?variables={"showId":"{show_id}"}&extensions={"persistedQuery":{"sha256Hash":"..."}}`
   * Output: JSON list of available sub and dub episode counts.
3. **Stream Source Query:**
   * `GET https://api.allanime.day/api?variables={"showId":"{show_id}","episodeString":"{ep_num}","translationType":"sub"}`
   * Output: JSON with `episode.sourceUrls` array.

### Deobfuscation Algorithm
* Source URLs starting with `--` are hex-encoded strings.
* Convert hex pairs back to ASCII characters to obtain the real CDN `.m3u8` link.

### Output
* Adaptive bitrate `.m3u8` playlist URL (Wixmp CDN).
* **Playback Header Required:** `Referer: https://allanime.to/`

---

## 8. OnePace (`onepace.net`)

### What It Expects
* **Series Arcs:** Clean, filler-free cut of One Piece.
* **REST API:** Public endpoints returning streaming and direct download files.

### API Endpoints & Request Flow
1. **Anime Arcs & Episodes:**
   * `GET https://onepace.net/api/arcs`
   * Output: JSON structure of all arcs, episode titles, and episode IDs.
2. **Stream / Download Resolution:**
   * Parse episode object to extract stream links:
     - Pixeldrain direct CDN URL.
     - Telegram CDN mirror.
     - Torrent magnet links.

### Output
* Direct MP4 file link with dual audio (Japanese + English) and embedded multi-subtitles.
* **Playback Header Required:** Standard `User-Agent`.

---

## 9. AnimeWorld India (`animeworldindia.net`) — *Hindi Dub*

### What It Expects
* **Search:** Series title.
* **Post Scraping:** WordPress HTML post parsing for multi-audio and episode tables.

### API Endpoints & Request Flow
1. **Search Query:**
   * `GET https://animeworldindia.net/?s={query}`
   * Output: HTML search results with post URLs.
2. **Episode Table Parsing:**
   * `GET {post_url}`
   * Output: HTML page containing tables categorized by language (Hindi Dub, Tamil, Telugu, English Sub).
3. **Extract Stream Host Links:**
   * Follow redirect / download links to storage hosts (GDrive, Mega, StreamWish, FileLions).
4. **Resolve Stream via Universal Host Extractor:**
   * If StreamWish (`streamwish.to`): Unpack Dean Edwards `eval(p,a,c,k,e,d)` script -> Extracts `.m3u8`.
   * If FileLions: Extract master playlist URL from player config.

### Output
* Direct `.m3u8` or MP4 video with Hindi dub audio.
* **Playback Header Required:** `Referer: https://streamwish.to/` (or matching host).

---

## 10. Toonworld4all (`toonworld4all.me`) — *Hindi Dub*

### What It Expects
* **Search:** Anime or cartoon name.
* **Episode Links:** Multi-quality download and stream tables.

### API Endpoints & Request Flow
1. **Search:**
   * `GET https://toonworld4all.me/?s={query}`
   * Output: HTML list of matching posts.
2. **Episode Page:**
   * `GET {post_url}`
   * Output: Table containing episodes with labels: `Hindi Dubbed [720p/1080p]`.
3. **Link Bypass / Resolution:**
   * Follow episode button to intermediate streaming redirector.
   * Extract target embed host (StreamWish / DoodStream / Direct Drive).
4. **Stream Extraction:**
   * Extract `.m3u8` from StreamWish unpacker.

### Output
* Playable HLS stream / MP4 file with Hindi audio track.
* **Playback Header Required:** Host-specific `Referer`.

---

## 11. DesiDubAnime (`desidubanime.me`) — *Hindi Dub*

### What It Expects
* **Search:** Anime name.
* **Episode Selection:** Season and episode numbers.

### API Endpoints & Request Flow
1. **Search:**
   * `GET https://desidubanime.me/?s={query}`
   * Output: HTML list of anime series posts.
2. **Watch Page:**
   * `GET https://desidubanime.me/{anime-slug}-episode-{num}/`
   * Output: HTML containing responsive video player iframe (`StreamWish` or `FileLions`).
3. **Iframe Stream Resolution:**
   * Load iframe source URL (e.g. `https://streamwish.to/e/{id}`).
   * Headers: `Referer: https://desidubanime.me/`
   * Parse JavaScript packer block: `eval(function(p,a,c,k,e,d)...)`.
   * Unpack to obtain `sources: [{file: "https://.../master.m3u8"}]`.

### Output
* Direct HLS master `.m3u8` stream with Hindi dub audio.
* **Playback Header Required:** `Referer: https://streamwish.to/`

---

## 12. AnimeTM (`animetm.net`) — *Hindi Dub*

### What It Expects
* **Search:** Title query.
* **Community Dubs:** Community-produced and official Hindi dub releases.

### API Endpoints & Request Flow
1. **Search:**
   * `GET https://animetm.net/?s={query}`
   * Output: HTML list of releases.
2. **Episode Post:**
   * `GET {post_url}`
   * Output: HTML post with internal video player iframe.
3. **Direct Video Resolution:**
   * Extract video tag `src` or JSON source definition pointing to CDN storage.

### Output
* Direct MP4 video stream link.
* **Playback Header Required:** `Referer: https://animetm.net/`

---

## 13. Official Channels: Muse India & Ani-One India — *Legal Free Hindi Dub*

### What It Expects
* **YouTube Video ID** or **Playlist ID**.
* **Extraction Tool:** `yt-dlp` or Invidious public REST API.

### API Endpoints & Request Flow
1. **Search Episodes via Invidious API:**
   * `GET https://api.invidious.io/api/v1/search?q={query}&channel=MuseIndia`
   * Output: JSON list of video IDs and episode numbers.
2. **Stream URL Resolution via `yt-dlp`:**
   * Execute: `yt-dlp -g -f best "https://www.youtube.com/watch?v={video_id}"`
   * Output: Direct Google video CDN `.m3u8` / HTTPS stream URL.

### Output
* High-speed, official 1080p stream with Hindi dub or English sub.
* **Zero anti-bot blocking or domain takedown risk.**

# APIs Used in `ani-cli`

This document details all external APIs, web scraping endpoints, stream extraction mechanisms, and remote data endpoints utilized in `ani-cli`.

---

## Table of Contents
1. [HiAnime APIs (`hianime.at`)](#1-hianime-apis-hianimeat)
   - [1.1 Search Anime API](#11-search-anime-api)
   - [1.2 Episode List API](#12-episode-list-api)
   - [1.3 Episode Servers API](#13-episode-servers-api)
2. [Embed Stream & Video Extraction](#2-embed-stream--video-extraction)
   - [2.1 Embed Page & Blob Extraction](#21-embed-page--blob-extraction)
   - [2.2 Blob Deobfuscation (ZokoAnime)](#22-blob-deobfuscation-zokoanime)
   - [2.3 Master M3U8 Playlist & Stream Quality Selection](#23-master-m3u8-playlist--stream-quality-selection)
3. [AnimeSchedule APIs (`animeschedule.net`)](#3-animeschedule-apis-animeschedulenet)
   - [3.1 Anime Search API (v3)](#31-anime-search-api-v3)
   - [3.2 Countdown Page Scrape](#32-countdown-page-scrape)
4. [GitHub Auto-Update API](#4-github-auto-update-api)
5. [External Subtitle Download & Integrations](#5-external-subtitle-download--integrations)
6. [Popular Free Anime Streaming APIs & Ecosystem Recommendations](#6-popular-free-anime-streaming-apis--ecosystem-recommendations)
   - [6.1 Top Free Anime Streaming Site APIs](#61-top-free-anime-streaming-site-apis)
   - [6.2 Best Recommendations for Developers](#62-best-recommendations-for-developers)
7. [In-Depth Multi-Tier Streaming Architecture & Link Resolution](#7-in-depth-multi-tier-streaming-architecture--link-resolution)
   - [7.1 The 3-Tier Streaming System Architecture](#71-the-3-tier-streaming-system-architecture)
   - [7.2 Detailed End-to-End Stream Resolution Pipeline](#72-detailed-end-to-end-stream-resolution-pipeline)
   - [7.3 Other Major Free Anime Streaming Networks](#73-other-major-free-anime-streaming-networks-under-the-hood)

---

## 1. HiAnime APIs (`hianime.at`)

Base URL: `https://hianime.at`

### 1.1 Search Anime API
Searches the HiAnime catalog for anime titles matching a query.

- **Endpoint:** `GET https://hianime.at/search`
- **Query Parameters:**
  - `keyword` *(string, required)*: The search query string (spaces replaced with `+`).
- **Headers:**
  - `User-Agent`: Modern browser User-Agent string.
  - Custom TLS Cipher Suites (if required by curl setup).
- **Response:** HTML page containing search results.
- **Extracted Data:**
  - Anime Slug / ID: Extracted from `<a href=".../{anime_id}">` inside `<h3 class="film-name">`.
  - Anime Title: Extracted from `title="..."` attribute inside `<h3 class="film-name">`.
- **Function in `ani-cli`:** `hianime_search()` (Line 199)

---

### 1.2 Episode List API
Retrieves the episode list for a specific anime by its numeric ID suffix.

- **Endpoint:** `GET https://hianime.at/api/theme/episode/list/{anime_id_numeric}`
- **Path Parameters:**
  - `anime_id_numeric` *(string/number, required)*: The numeric suffix extracted from the anime slug (e.g. `12345` from `one-piece-12345`).
- **Response:** HTML fragment containing episode element nodes (`ep-item`).
- **Extracted Data:**
  - Episode Number: Extracted from `data-number="..."`.
  - Episode ID: Extracted from `data-id="..."`.
- **Function in `ani-cli`:** `hianime_episodes()` (Line 213)

---

### 1.3 Episode Servers API
Fetches available streaming servers and encrypted stream hashes for a given episode ID.

- **Endpoint:** `GET https://hianime.at/api/theme/episode/servers`
- **Query Parameters:**
  - `episodeId` *(string/number, required)*: Numeric ID of the target episode.
- **Response:** HTML fragment containing server item definitions (`server-item`).
- **Extracted Data:**
  - Server Hash (`data-hash`): Base64-encoded embed string for the requested mode (`sub` or `dub`) using the `ZokoAnime` server.
- **Function in `ani-cli`:** `hianime_m3u8()` (Line 222)

---

## 2. Embed Stream & Video Extraction

### 2.1 Embed Page & Blob Extraction
Decodes the embed link from the server hash and fetches the player configuration.

- **Embed URL:** Base64-decoded string from `data-hash` (e.g. `https://{embed_host}/embed/.../mal/{mal_id}/...`).
- **Method:** `GET {embed_url}`
- **Headers:**
  - `Referer`: `{embed_host_origin}/`
- **Extracted Data:**
  - `mal_id`: MyAnimeList ID parsed from embed URL pattern `/mal/{mal_id}/` (used for `ani-skip`).
  - Obfuscated Config Blob: Extracted from `window.__P="{blob}"` in the response HTML.

---

### 2.2 Blob Deobfuscation (ZokoAnime)
The embed page stores stream configuration inside an obfuscated Base64 string (`window.__P`).

- **Deobfuscation Algorithm:**
  1. Base64 decode `window.__P`.
  2. XOR each byte with the repeating ASCII key bytes of `"otaku-embed-v1"` (byte values: `111, 116, 97, 107, 117, 45, 101, 109, 98, 101, 100, 45, 118, 49`).
  3. Parse the resulting string as JSON.
- **Extracted Fields from Decoded JSON:**
  - `src`: Master `.m3u8` stream URL.
  - `subtitles`: Array of subtitle objects; extracts `src` where `"default": true` for VTT subtitles.
- **Function in `ani-cli`:** `deobfuscate_blob()` (Line 185)

---

### 2.3 Master M3U8 Playlist & Stream Quality Selection
Fetches the master HLS playlist to resolve individual stream resolutions.

- **Endpoint:** `GET {m3u8_master_url}`
- **Headers:**
  - `Referer`: Stream host base referer URL (`refr`).
- **Response:** Standard HLS M3U8 playlist with resolution tags (`#EXT-X-STREAM-INF`).
- **Quality Options Handled:** `best`, `worst`, or specific resolutions (e.g., `1080p`, `720p`, `480p`, `360p`).
- **Function in `ani-cli`:** `select_quality()` (Line 245) & `hianime_m3u8()` (Line 222)

---

## 3. AnimeSchedule APIs (`animeschedule.net`)

Base URL: `https://animeschedule.net`

### 3.1 Anime Search API (v3)
Searches AnimeSchedule for release schedule information.

- **Endpoint:** `GET https://animeschedule.net/api/v3/anime`
- **Query Parameters:**
  - `q` *(string, required)*: Anime search query string.
- **Response:** JSON array of anime schedule objects.
- **Extracted Data:**
  - `route`: Relative path URL slug for the anime detail page on AnimeSchedule.
  - `english`: English title.
  - `title` / `romaji`: Romaji/Japanese title.
  - `status`: Airing status (e.g., `Ongoing`, `Finished`).
- **Function in `ani-cli`:** `time_until_next_ep()` (Line 270)

---

### 3.2 Countdown Page Scrape
Scrapes the anime detail page on AnimeSchedule to extract release countdown timers for upcoming subbed and dubbed episodes.

- **Endpoint:** `GET https://animeschedule.net/anime/{route}`
- **Response:** HTML web page.
- **Extracted Data:**
  - Next Subbed Episode Number: Parsed from `release-time-type-subs`.
  - Next Dubbed Episode Number: Parsed from `release-time-type-dub`.
  - Sub / Dub Countdown Datetime: Parsed from `class="countdown-time"` datetime attributes.
- **Function in `ani-cli`:** `time_until_next_ep()` (Line 294)

---

## 4. GitHub Auto-Update API

Checks for script updates and patches `ani-cli` to the latest version.

- **Endpoint:** `GET https://raw.githubusercontent.com/pystardust/ani-cli/{branch}/ani-cli`
- **Path Parameters:**
  - `branch` *(string)*: Git branch (default: `master`).
- **Response:** Plain text shell script (`ani-cli`).
- **Extracted Data:**
  - `version_number`: Parsed using regex `s|^version_number="([^"]+)"$|\1|p`.
  - Code Diff: Used with `diff -u` and `patch` to update local script.
- **Function in `ani-cli`:** `update_script()` (Line 110)

---

## 5. External Subtitle Download & Integrations

- **VTT Subtitles Endpoint:** `GET {sub_link}`
  - **Headers:** `User-Agent`, `Referer: {refr}`
  - **Purpose:** Downloads WebVTT subtitle track for video players / local download.
  - **Function in `ani-cli`:** `download()` (Line 348)
- **`ani-skip` Integration:**
  - Executes `ani-skip -i {mal_id} -e {ep_no}` locally to fetch intro skip timestamps (for `mpv`).

---

## 6. Popular Free Anime Streaming APIs & Ecosystem Recommendations

### 6.1 Top Free Anime Streaming Site APIs

| API / Provider | Type | Requires Login? | Video Format | Features |
| :--- | :--- | :--- | :--- | :--- |
| **HiAnime (Zoro)** | Web Scraper / API | No | `.m3u8` (HLS 1080p) | Sub & Dub, high-speed servers (MegaCloud/ZokoAnime), intro/outro timestamps |
| **Gogoanime** | Web Scraper / API | No | `.m3u8` / `.mp4` | Huge legacy & current anime library, multiple mirror hosts (Vidstreaming, Streamtape) |
| **AnimePahe** | Web Scraper / API | No | `.m3u8` / `.mp4` | Highly compressed streams (low bandwidth, crisp 720p/1080p via Kwik player) |
| **AllAnime** | Web Scraper / GraphQL | No | `.m3u8` | Fast GraphQL search & episode link resolution, sub/dub options |
| **Consumet API** | Open-Source REST API | No (Self-Hostable) | Unified JSON (`.m3u8`) | Aggregates HiAnime, Gogoanime, AnimePahe, and Crunchyroll into standard REST endpoints |

---

### 6.2 Best Recommendations for Developers

1. **Best Overall Free API (Easiest to Use): Consumet API**
   - **Why:** Wraps scrapers for Gogoanime, Zoro/HiAnime, AnimePahe, and FlixHQ into a clean, ready-to-use REST API (`/anime/gogoanime/info/{id}`, `/anime/zoro/watch`).
   - **Cost:** 100% Free & Open-Source (Can be self-hosted on Vercel/Render).

2. **Best High-Quality Stream API: HiAnime (Zoro)**
   - **Why:** Provides fast multi-quality HLS streams (up to 1080p 60fps), embedded WebVTT subtitles, and MAL ID mappings for skipping intros.

3. **Best Metadata & Tracking APIs (Free, No Auth Required):**
   - **AniList GraphQL API** (`https://graphql.anilist.co`): Best for search, cover art, anime metadata, and episode counts.
   - **Jikan API (MyAnimeList v4)** (`https://api.jikan.moe/v4`): Official unofficial REST wrapper for MyAnimeList database.
   - **AniSkip API** (`https://api.aniskip.com/v2`): Free API for fetching opening (`op`) and ending (`ed`) skip timestamps.

---

## 7. In-Depth Multi-Tier Streaming Architecture & Link Resolution

Modern free anime streaming platforms operate using a decoupled 3-tier system to isolate frontend metadata, embed video players, and high-bandwidth CDN video delivery nodes.

### 7.1 The 3-Tier Streaming System Architecture

```
+-------------------------------------------------------------------------------+
|                        TIER 1: FRONTEND WEBSITES                              |
|           (e.g., aniwatch.lu, hianime.at, hianime.to)                         |
|  * Handles anime title searches                                               |
|  * Renders UI, metadata, episode lists, sub/dub selection                      |
|  * Endpoint: GET /api/theme/episode/servers?episodeId={ep_id}                 |
+-------------------------------------------------------------------------------+
                                      |
                                      | Base64 Encoded Embed Hash (data-hash)
                                      v
+-------------------------------------------------------------------------------+
|                       TIER 2: EMBED PLAYER HOSTS                              |
|           (e.g., zokoanime.video, megaplay.buzz, megacloud.tv)                |
|  * Hosts the video player iframe & blob generator                              |
|  * Serves HTML containing obfuscated config: window.__P="{base64_blob}"       |
|  * Extracted MAL ID for intro/outro skipping                                  |
+-------------------------------------------------------------------------------+
                                      |
                                      | XOR Deobfuscated JSON (key: "otaku-embed-v1")
                                      v
+-------------------------------------------------------------------------------+
|                     TIER 3: EDGE CDN STREAMING NODES                          |
|    (e.g., hls.dramahot.top, wlxos.silverorbit.top, leltb.wintergrove.space)   |
|  * Master HLS Playlist: /v/.../master.m3u8                                    |
|  * Quality Variant Playlists: 1080/index.m3u8, 720/index.m3u8, 360/index.m3u8 |
|  * Video Segments: /.../seg_00015.ts                                          |
|  * Scrubbing Thumbnails: /.../seg-1-f1-v1-a1.jpg                             |
+-------------------------------------------------------------------------------+
```

---

### 7.2 Detailed End-to-End Stream Resolution Pipeline

Taking **`aniwatch.lu` / `hianime.at`** as a live example, here is how anime details are transformed into playable video streams:

#### Step 1: Anime Search & Detail Resolution
- **Request:** `GET https://hianime.at/search?keyword=one+piece`
- **Output:** HTML response containing anime ID slug (e.g. `one-piece-1`).

#### Step 2: Episode List Retrieval
- **Request:** `GET https://hianime.at/api/theme/episode/list/1`
- **Output:** HTML list containing episode elements with `data-id` attributes (e.g., `data-id="149894"` for Episode 1180).

#### Step 3: Fetch Server Hash
- **Request:** `GET https://hianime.at/api/theme/episode/servers?episodeId=149894`
- **Output:** HTML JSON response with server items (`server-item`):
  - Server: `ZokoAnime`
  - `data-hash`: `aHR0cHM6Ly96b2tvYW5pbWUudmlkZW8vc3RyZWFtL21hbC8yMS8xMTgwL3N1Yg==`

#### Step 4: Decode Embed URL
- **Process:** Base64 decoding `data-hash` yields:
  - Embed URL: `https://zokoanime.video/stream/mal/21/1180/sub` (or `https://megaplay.buzz/stream/s-2/...`)
  - MAL ID: `21` (passed to `AniSkip API` for intro/outro skip intervals).

#### Step 5: Extract Obfuscated Blob
- **Request:** `GET https://zokoanime.video/stream/mal/21/1180/sub` (Header: `Referer: https://zokoanime.video/`)
- **Output:** HTML script containing `window.__P="{blob_string}"`.

#### Step 6: Deobfuscate Blob Payload
- **Algorithm:**
  1. Base64 decode `{blob_string}` into byte array.
  2. Perform XOR on each byte `i` using repeating key `"otaku-embed-v1"` (byte array: `[111, 116, 97, 107, 117, 45, 101, 109, 98, 101, 100, 45, 118, 49]`).
  3. Parse result as JSON.
- **Output JSON:**
  ```json
  {
    "src": "https://hls.dramahot.top/v/.../master.m3u8",
    "subtitles": [
      { "src": "https://.../sub.vtt", "default": true, "label": "English" }
    ]
  }
  ```

#### Step 7: Stream HLS Video Chunks from CDN
- **Master Playlist:** `GET https://hls.dramahot.top/v/.../master.m3u8` (Referer: `https://zokoanime.video/`)
- **Quality Variants:**
  - `1080p`: `https://hls.dramahot.top/v/.../1080/index.m3u8`
  - `720p`: `https://hls.dramahot.top/v/.../720/index.m3u8`
  - `360p`: `https://hls.dramahot.top/v/.../360/index.m3u8`
- **Video Transport Segments:** `GET https://hls.dramahot.top/v/.../360/seg_00015.ts` (Actual MPEG-TS video data chunks).
- **Trickplay Preview Thumbnails:** `GET https://wlxos.silverorbit.top/anime/.../seg-1-f1-v1-a1.jpg` (Sprite image for player seek bar preview).

---

### 7.3 Other Major Free Anime Streaming Networks (Under the Hood)

#### 1. **GogoAnime / AniTaku Network (`gogoanime3.co`, `anitaku.pe`)**
* **Frontend:** `gogoanime3.co` / `anitaku.pe`
* **Embed Hosts:** `gogohd.net`, `vidstreaming.io`, `streamtape.com`
* **CDN Nodes:** `gogocdn.net`
* **How It Works:** Parses iframe `src` from episode page, extracts URL query params (`id`, `time`), decrypts parameters using AES-CBC with secret keys, and requests `.m3u8` links from `Vidstreaming` API.

#### 2. **AnimePahe Network (`animepahe.ru`)**
* **Frontend:** `animepahe.ru`
* **Embed Host:** `kwik.cx`
* **CDN Nodes:** `pahe.win`, `kwik.cx`
* **How It Works:** Obtains episode session ID via API, loads `Kwik` player iframe, unpacks obfuscated JavaScript (`eval(function(p,a,c,k,e,d)...)`), and extracts direct HLS playlist URL.

#### 3. **AllAnime Network (`allanime.day`)**
* **Frontend:** `allanime.day`
* **Embed Hosts:** `wixmp`, `gdriveplayer`, `dropload`
* **CDN Nodes:** `wixmp.com`
* **How It Works:** Issues GraphQL query to `https://api.allanime.day/api`, retrieves hex/base64 encoded player source links, decodes them to HLS streams, and renders custom HTML5 player controls.



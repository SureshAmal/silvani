# Anime Streaming Providers & Data Architecture Research

This document provides a comprehensive research breakdown of the video streaming providers used across open-source anime scrapers (such as `ani-cli`, `Consumet`, `Aniyomi / Yuzono`, `Miru`, and `Animdl`). It defines all available providers, their extraction mechanisms, and the exact data fields required to build a video streaming and downloading application.

---

## Table of Contents
1. [Core Requirements & Workflow](#1-core-requirements--workflow)
2. [Complete Provider Ecosystem Comparison](#2-complete-provider-ecosystem-comparison)
3. [Deep-Dive into Top Streaming Providers](#3-deep-dive-into-top-streaming-providers)
   - [3.1 HiAnime / AniWatch Network](#31-hianime--aniwatch-network)
   - [3.2 GogoAnime / AniTaku Network](#32-gogoanime--anitaku-network)
   - [3.3 AnimePahe Network](#33-animepahe-network)
   - [3.4 AllAnime Network](#34-allanime-network)
   - [3.5 KickAssAnime (KAA)](#35-kickassanime-kaa)
   - [3.6 Regional / Multi-Language Providers](#36-regional--multi-language-providers)
4. [Video Embed Hosts & Extraction Methods](#4-video-embed-hosts--extraction-methods)
5. [Unified Data Model Specifications](#5-unified-data-model-specifications)
   - [5.1 Search & Catalog Item](#51-search--catalog-item)
   - [5.2 Episode List Item](#52-episode-list-item)
   - [5.3 Stream Payload (Video, Audio, Subtitles, Resolutions)](#53-stream-payload-video-audio-subtitles-resolutions)
6. [Architectural Recommendations for Rust / GPUI](#6-architectural-recommendations-for-rust--gpui)
7. [FMHY Curated Providers & Open-Source Client Reference](#7-fmhy-curated-providers--open-source-client-reference)
   - [7.1 Notable Open-Source Anime Streaming Apps on FMHY](#71-notable-open-source-anime-streaming-apps-on-fmhy)
   - [7.2 Top-Starred ("⭐") Sites on FMHY](#72-top-starred--sites-on-fmhy)
8. [HLS Segment Proxies & Edge Relays (Case Study: anistream.one & megaplay.buzz)](#8-hls-segment-proxies--edge-relays-case-study-anistreamone--megaplaybuzz)
   - [8.1 URL Breakdown](#81-url-breakdown)
   - [8.2 Why Providers Use Segment Proxies](#82-why-providers-use-segment-proxies)
   - [8.3 Live Inspection Results](#83-live-inspection-results)

---

## 1. Core Requirements & Workflow

To stream and download anime reliably, an application must execute a 3-step pipeline across any provider:

```
[Step 1: Search Query] ──> [Step 2: Episode List] ──> [Step 3: Stream Payload]
   - Anime Title              - Episode Numbers          - Sub / Dub Selection
   - Unique Provider Slug     - Internal Episode IDs     - Multi-Quality Resolutions (1080p, 720p, etc.)
   - Poster Image & Status    - Server Options           - Soft Subtitle Tracks (.vtt)
                                                         - Required HTTP Headers (Referer)
```

---

## 2. Complete Provider Ecosystem Comparison

Below is the matrix of streaming providers utilized in top open-source projects:

| Provider Name | Used In | Audio Options | Subtitle Type | Stream Format | Resolutions | Primary Video Hosts | Anti-Scrape Level |
| :--- | :--- | :--- | :--- | :--- | :--- | :--- | :--- |
| **HiAnime (AniWatch)** | `ani-cli`, Consumet | Sub & Dub | Softsub (WebVTT) | HLS (`.m3u8`) | 1080p, 720p, 360p | ZokoAnime, MegaCloud, Vidstream | Medium (Base64 + XOR) |
| **GogoAnime (AniTaku)** | Consumet, Aniyomi, Miru | Sub & Dub (Separate titles) | Hardsub (Baked-in) | HLS & MP4 | 1080p, 720p, 480p, 360p | Vidstreaming, GogoCDN, Streamtape | Medium (AES-CBC encrypted query) |
| **AnimePahe** | Consumet, Aniyomi, Animdl | Sub & Dub | Hardsub (Sub) / English (Dub) | HLS (`.m3u8`) | 1080p, 720p (High Compression) | Kwik (`kwik.cx`) | Medium (JS Packing / `eval`) |
| **AllAnime** | Aniyomi, Consumet | Sub & Dub (Per-episode toggle) | Softsub & Hardsub | HLS (`.m3u8`) | 1080p, 720p, 480p, 360p | Wixmp, DropLoad, Swish | Low-Medium (GraphQL + Hex Decode) |
| **KickAssAnime** | Consumet, Aniyomi | Sub & Dub + Multi-Audio | Softsub (Multi-Lang VTT) | HLS (`.m3u8`) | 1080p, 720p, 480p, 360p | DuckStream, BirdStream | High (Custom AES API) |
| **AnimeFLV** (Spanish) | Aniyomi | Spanish Sub (VOST) | Hardsub | MP4 & HLS | 1080p, 720p, 480p | Streamtape, Mega, YourUpload | Low (Direct iframes) |

---

## 3. Deep-Dive into Top Streaming Providers

### 3.1 HiAnime / AniWatch Network
* **Active Domains:** `hianime.at`, `aniwatch.lu`, `hianime.to`
* **How Search Works:** `GET /search?keyword={query}`. HTML parsing of `.film-detail .film-name a` extracts the anime slug (e.g. `one-piece-1`).
* **How Episode List Works:** `GET /api/theme/episode/list/{numeric_id}`. Returns an HTML snippet with `data-number` and `data-id`.
* **Streaming Capability:**
  * **Audio Options:** Explicit Sub and Dub buttons on each episode.
  * **Subtitles:** True softsubtracks (`.vtt`) in English and occasional other languages.
  * **Resolutions:** Master `.m3u8` containing `1080p`, `720p`, `360p`.
  * **Deobfuscation:** Server hash is Base64-decoded into an embed URL (`zokoanime.video`), which exposes `window.__P`. The blob is XOR-decoded with key `otaku-embed-v1`.

### 3.2 GogoAnime / AniTaku Network
* **Active Domains:** `anitaku.pe`, `gogoanime3.co`, `gogoanime.cl`
* **How Search Works:** `GET /search.html?keyword={query}`. HTML parsing extracts anime link slugs. Dubbed anime are typically cataloged under separate slugs (e.g. `/category/naruto-dub`).
* **How Episode List Works:** Uses an internal AJAX endpoint (`/load-list-episode?ep_start=0&ep_end=...&id=...`) to return the complete episode listing.
* **Streaming Capability:**
  * **Audio Options:** Japanese (Sub) or English (Dub) depending on the series slug.
  * **Subtitles:** Hardsubs burned directly into the video stream for Japanese audio.
  * **Resolutions:** Direct MP4 files and HLS master playlists (`1080p`, `720p`, `480p`, `360p`).
  * **Deobfuscation:** Requires parsing the `Vidstreaming` / `GogoCDN` iframe URL, extracting encrypted parameters, and decrypting via AES-CBC using known community keys to fetch JSON stream sources.

### 3.3 AnimePahe Network
* **Active Domains:** `animepahe.ru`, `animepahe.org`
* **How Search Works:** `GET /api?m=search&q={query}`. Returns clean JSON data with anime session IDs and poster URLs.
* **How Episode List Works:** `GET /api?m=release&id={session_id}&sort=asc&page={page}`. Returns JSON paginated episode lists.
* **Streaming Capability:**
  * **Audio Options:** Sub and Dub variants labeled on episode download buttons.
  * **Subtitles:** Hardcoded English subtitles.
  * **Resolutions:** 720p and 1080p encoded at very efficient bitrates (approx. 100MB - 250MB per episode).
  * **Deobfuscation:** Follows Kwik redirect URLs (`https://kwik.cx/e/...`), unpacks Dean Edwards `p,a,c,k,e,d` JavaScript, and extracts the direct `.m3u8` link.

### 3.4 AllAnime Network
* **Active Domains:** `allanime.day`, `allanime.to`
* **How Search Works:** Native GraphQL queries to `https://api.allanime.day/api` specifying title queries, adult filters, and pagination.
* **How Episode List Works:** GraphQL query requesting available episodes, with separate counts for Subbed and Dubbed episodes.
* **Streaming Capability:**
  * **Audio Options:** Both Sub and Dub options selectable on the same episode object.
  * **Subtitles:** Both softsub WebVTT tracks and hardsub streams.
  * **Resolutions:** Adaptive HLS (360p up to 1080p).
  * **Deobfuscation:** Decodes hex-encoded strings into direct CDN endpoints.

---

## 4. Video Embed Hosts & Extraction Methods

Streaming sites do not host video files on their web servers. They delegate video delivery to external embed hosts. Here are the primary embed players encountered:

| Embed Host | Used By | Decryption / Deobfuscation Required | Supported Video Protocols | Hotlink Header Needed |
| :---: | :---: | :---: | :---: | :---: |
| **ZokoAnime** (`zokoanime.video`) | HiAnime / AniWatch | Base64 + XOR key `otaku-embed-v1` | HLS (`.m3u8`) | `Referer: https://zokoanime.video/` |
| **MegaCloud / RapidCloud** (`megacloud.tv`, `megaplay.buzz`) | HiAnime / Kaido | Client-side cipher extraction | HLS (`.m3u8`) | `Referer: https://megacloud.tv/` |
| **Vidstreaming / GogoCDN** | GogoAnime | AES-256-CBC query decryption | HLS (`.m3u8`) + Direct MP4 | `Referer: https://anitaku.pe/` |
| **Kwik** (`kwik.cx`) | AnimePahe | JS Packer unpacker (`eval(p,a,c,k,e,d)`) | HLS (`.m3u8`) | `Referer: https://kwik.cx/` |
| **Streamtape** (`streamtape.com`) | GogoAnime, AnimeFLV | Robot token extraction from DOM | Direct MP4 | `Referer: https://streamtape.com/` |
| **Filemoon** (`filemoon.sx`) | GogoAnime, KickAssAnime | JS Packer unpacker | HLS (`.m3u8`) | `Referer: https://filemoon.sx/` |

---

## 5. Unified Data Model Specifications

To build a clean Rust application with GPUI, all providers should conform to a single set of standardized data structures.

### 5.1 Search & Catalog Item
Represents an anime entry in search results or the latest releases grid:

```
AnimeCatalogItem:
  - id: String                   # Unique provider slug (e.g., "one-piece-1")
  - provider: String             # "hianime" | "gogoanime" | "animepahe"
  - title: String                # Main display title
  - alternate_titles: List<String> # Japanese / Romaji / English titles
  - poster_url: String           # Image cover thumbnail
  - has_sub: Boolean             # Whether subbed episodes exist
  - has_dub: Boolean             # Whether dubbed episodes exist
  - latest_episode: Option<Float> # e.g., 1180.0
```

### 5.2 Episode List Item
Represents an individual episode available for playback or download:

```
EpisodeItem:
  - episode_id: String           # Provider internal ID (e.g., "149894")
  - episode_number: Float        # 1.0, 2.0, 10.5 (handles decimals & specials)
  - title: Option<String>        # "I'm Luffy! The Man Who Will Become the Pirate King!"
  - thumbnail_url: Option<String># Episode preview frame
  - has_sub: Boolean             # True if sub exists
  - has_dub: Boolean             # True if dub exists
```

### 5.3 Stream Payload (Video, Audio, Subtitles, Resolutions)
The complete payload returned when requesting a stream for an episode:

```
StreamPayload:
  - provider: String             # e.g., "hianime"
  - audio_mode: AudioMode        # Sub | Dub
  - audio_language: String       # "Japanese" (for Sub) | "English" (for Dub)
  - stream_format: StreamFormat  # HlsMaster (.m3u8) | DirectFile (.mp4)
  
  # Multi-Quality Resolutions
  - quality_options: List<QualityVariant>
      * label: String            # "1080p", "720p", "480p", "360p", "Auto"
      * url: String              # Sub-playlist URL or direct video URL
      * resolution: Option<(u32, u32)> # (1920, 1080)
      * bandwidth_bps: Option<u64>     # e.g., 5300000
      
  # Subtitle Tracks
  - is_hardsub: Boolean          # True if text is burned into the video
  - subtitles: List<SubtitleTrack>
      * language: String         # "English", "Spanish", etc.
      * url: String              # WebVTT (.vtt) or SRT (.srt) link
      * is_default: Boolean      # True if default active track
      
  # Anti-Hotlinking Headers (Mandatory for Playback & Download)
  - headers: Map<String, String>
      * "Referer": "https://zokoanime.video/"
      * "User-Agent": "Mozilla/5.0 ..."
```

---

## 6. Architectural Recommendations for Rust / GPUI

1. **Provider Trait Design (Extensibility):**
   * Define an asynchronous Rust trait:
     ```
     trait AnimeProvider: Send + Sync {
         async fn search(&self, query: &str) -> Result<Vec<AnimeCatalogItem>>;
         async fn get_episodes(&self, anime_id: &str) -> Result<Vec<EpisodeItem>>;
         async fn get_stream(&self, episode_id: &str, mode: AudioMode, quality: &str) -> Result<StreamPayload>;
     }
     ```
   * Implement this trait for `HiAnimeProvider`, `GogoAnimeProvider`, and `AnimePaheProvider`.

2. **Provider Prioritization:**
   * **Primary Provider:** **HiAnime** — It offers the cleanest separation of Sub/Dub, high-speed 1080p streams via HLS, soft WebVTT subtitles, and intro skip MAL IDs.
   * **Fallback 1:** **AnimePahe** — Best for fast downloads and smaller file sizes.
   * **Fallback 2:** **GogoAnime** — Largest historical anime library for older series that might not be on HiAnime.

3. **Decoupling Playback and Downloading:**
   * Both the video player engine and the file downloader only need the final `StreamPayload` (stream URL + resolution + Referer headers).
   * Playback passes the stream URL and headers directly to the video player.
   * Downloader passes the stream URL to an asynchronous segment fetcher to assemble the local `.mp4` file.

---

## 7. FMHY Curated Providers & Open-Source Client Reference

The **[FMHY (FreeMediaHeckYeah)](https://fmhy.net/video#anime-streaming)** index is the premier community catalog for anime streaming and open-source applications. Below are the top-rated apps and providers curated by FMHY:

### 7.1 Notable Open-Source Anime Streaming Apps on FMHY
* **`ani-cli`** ([GitHub](https://github.com/pystardust/ani-cli)): Minimalist Unix CLI player (Linux, macOS, Android/Termux, Windows) using HiAnime scraper + MPV.
* **`Miru`** ([GitHub](https://github.com/miru-project/miru-app)): Desktop/Android streaming app built with TypeScript and Flutter/Rust, supporting extension scrapers and BitTorrent streams.
* **`Seanime`** ([GitHub](https://github.com/5rahim/seanime)): Desktop media server and player written in Go + React, integrating AniList, torrents, and HLS web scrapers.
* **`AnymeX`** ([GitHub](https://github.com/RyanYuuki/AnymeX)): Cross-platform desktop anime client built on the Mangayomi extension engine.

### 7.2 Top-Starred ("⭐") Sites on FMHY
* ⭐ **Miruro** (`miruro.com` / `miruro.tv`): Highly rated for both Soft Sub and Dub streams with an open-source frontend codebase ([GitHub](https://github.com/Miruro-no-kuon/Miruro)).
* ⭐ **AnimePahe** (`animepahe.pw` / `animepahe.ru`): Minimalist, ultra-efficient HLS bandwidth usage, Hard Subs and Dubs.
* ⭐ **KickAssAnime** (`kaa.lt`): Fast release schedule, multi-audio (Sub/Dub), and Soft VTT subtitles.
* ⭐ **AllManga / MKissa** (`allmanga.to` / `mkissa.to`): Web frontends powered by the AllAnime GraphQL API.
* ⭐ **Anikoto / AnimeSuge** (`anikototv.to` / `animesuge.cz`): Fast multi-server HLS streaming.

---

## 8. HLS Segment Proxies & Edge Relays (Case Study: `anistream.one` & `megaplay.buzz`)

In modern streaming architectures, platforms frequently route video traffic through **HLS Segment Proxies / Edge Workers** (e.g. `minky.anistream.one`) rather than having clients hit raw CDN storage directly.

### 8.1 URL Breakdown
```
https://minky.anistream.one/?u=G0cXAkBOcEQC...&origin=https%3A%2F%2Fmegaplay.buzz
```
* **Host:** `minky.anistream.one` — A Cloudflare-backed Edge Worker / Reverse Proxy.
* **`u` Parameter:** An encrypted/obfuscated token containing the upstream video storage chunk path.
* **`origin` Parameter:** The embed host origin (`https://megaplay.buzz`), telling the proxy which site initiated the request.

### 8.2 Why Providers Use Segment Proxies
1. **CORS Injection:** Web browsers enforce Cross-Origin Resource Sharing (CORS). Edge proxies inject `Access-Control-Allow-Origin: *` headers dynamically, allowing video players on any domain to load the stream.
2. **Hiding Origin Storage:** Obfuscates raw backend storage buckets (S3 / Wasabi / Backblaze) behind edge caching.
3. **ISP & Ad-Block Bypass:** Cloudflare edge caching minimizes latency and bypasses regional domain-level DNS blocks.

### 8.3 Live Inspection Results
* **Response Content-Type:** `video/mp2t` (MPEG-2 Transport Stream).
* **Payload Size:** ~1.86 MB (standard 4-second video chunk).
* **Sync Byte:** `0x47` (`G`), conforming strictly to the MPEG-TS specification.
* **Playback / Download Implication:** Video players (MPV) and HLS downloaders consume these segment URLs sequentially from the sub-playlist (`.m3u8`) without requiring extra decryption steps, provided the `Referer` and `Origin` headers are set.



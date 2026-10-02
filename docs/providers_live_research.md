# Live Verification & Streaming API Status for All Providers

Empirical testing results verifying live status, extraction APIs, and video availability for all providers in `multi_language_providers.md`.

---

## 1. English Sub (Japanese Audio) & English Dub

| Provider | Live Status | Streaming Host / Protocol | API / Extraction Method | Can Get Video? |
| :--- | :--- | :--- | :--- | :--- |
| **HiAnime / AniWatch** (`hianime.at`) | 🟢 **LIVE (200 OK)** | ZokoAnime / MegaCloud (`.m3u8`) | Base64 + XOR key `otaku-embed-v1` | **YES (1080p, 720p, Sub/Dub, VTT)** |
| **GogoAnime (AniTaku)** (`anitaku.pe`) | 🟢 **LIVE (200 OK)** | Vidstreaming / GogoCDN (`.m3u8` / MP4) | AES-256-CBC decrypt query token | **YES (1080p, Sub/Dub)** |
| **KickAssAnime** (`kaa.lt`) | 🟢 **LIVE (200 OK)** | DuckStream (`.m3u8`) | POST `/api/show/...` + AES decrypt | **YES (Multi-audio & softsubs)** |
| **AnimeSuge / Anikoto** (`anikototv.to`) | 🟢 **LIVE (200 OK)** | RapidStream / Vidstream (`.m3u8`) | Server list AJAX -> Iframe scrape | **YES (1080p HLS)** |
| **Re:ANIME** (`reanime.to`) | 🟢 **LIVE (200 OK)** | Internal HLS CDN (`.m3u8`) | DOM scrape `sources` array | **YES (HLS)** |
| **YugenAnime** (`yugenanime.tv`) | 🟢 **LIVE (200 OK)** | Yugen CDN (`.m3u8`) | POST `/api/embed/` | **YES (1080p)** |
| **AniWave Mirror** (`aniwaves.ru`) | 🟢 **LIVE (200 OK)** | Filemoon / Streamwish | Embed iframe JS unpacker | **YES (HLS)** |
| **AnimeOnsen** (`animeonsen.xyz`) | 🟢 **LIVE (200 OK)** | Direct MP4 / HLS 720p | REST API `/api/v1/anime/...` | **YES (Ad-free direct streams)** |
| **Senshi** (`senshi.to`) | 🟢 **LIVE (200 OK)** | Multi-server HLS | Iframe extractor | **YES (1080p)** |
| **AniZone** (`anizone.to`) | 🟢 **LIVE (200 OK)** | HLS CDN | DOM scrape `.m3u8` source | **YES (Sub)** |
| **AniHQ** (`anihq.cc`) | 🟢 **LIVE (200 OK)** | HLS CDN | AJAX stream token extraction | **YES (Sub/Dub)** |
| **AnimeNexus** (`anime.nexus`) | 🟢 **LIVE (200 OK)** | HLS CDN | Direct playlist URL in player state | **YES (Sub/Dub)** |
| **Animotvslash** (`animotvslash.org`) | 🟢 **LIVE (200 OK)** | Multi-host | Streamwish / Doodstream scraper | **YES** |
| **AnimeStream** (`anime.uniquestream.net`) | 🟢 **LIVE (200 OK)** | UniqueStream HLS | Iframe scraper | **YES** |
| **AnimeXin** (`animexin.dev`) | 🟢 **LIVE (200 OK)** | Multi-host HLS (Donghua) | JS unpacker | **YES (Asian/Donghua sub)** |
| **OnePace** (`onepace.net`) | 🟢 **LIVE (200 OK)** | Pixeldrain / Telegram / Torrent | Direct CDN MP4 links | **YES (One Piece clean cuts)** |
| **AllAnime** (`allanime.day`) | 🟡 **SHIELDED (301)** | Wixmp (`.m3u8`) | GraphQL API (`api.allanime.day/api`) | **YES (GraphQL hex decode)** |
| **WCOStream** (`wcostream.tv`) | 🟡 **SHIELDED (403 Cloudflare)**| WCO Internal CDN (MP4) | Base64 decode iframe | **YES (Requires browser headers)** |
| **Miruro** (`miruro.tv`) | 🔴 **DOWN/BLOCKED** | MegaCloud | Open-source API (`Miruro-no-kuon`) | ⚠️ Domain rotated, use HiAnime |
| **AnimePahe** (`animepahe.ru`) | 🔴 **DOWN/BLOCKED** | Kwik (`kwik.cx`) | Kwik JS unpacker (`eval(p,a,c,k,e,d)`) | ⚠️ Blocked in region, use proxy/mirror |

---

## 2. Hindi Dubbed Anime Sites

| Provider | Live Status | Streaming Host / Protocol | API / Extraction Method | Can Get Video? |
| :--- | :--- | :--- | :--- | :--- |
| **AnimeWorld India** (`animeworldindia.net`) | 🟢 **LIVE (200 OK)** | GDrive / Mega / Direct MP4 | Scrape post download/embed buttons | **YES (Direct MP4 links)** |
| **Toonworld4all** (`toonworld4all.me`) | 🟢 **LIVE (200 OK)** | Multi-host / GDrive / Streamwish | `TatakaiAPI` scraper / Direct links | **YES (Hindi dub MP4)** |
| **AnimeTM** (`animetm.net`) | 🟢 **LIVE (200 OK)** | Internal video server | Direct MP4 URLs | **YES (Fan-dub & official)** |
| **DesiDubAnime** (`desidubanime.me`) | 🟢 **LIVE (200 OK)** | Streamwish / FileLions | JS unpacker on iframe | **YES (Hindi, Tamil, Telugu)** |
| **Muse India** (`youtube.com/@MuseIndia`) | 🟢 **LIVE (Official)** | YouTube HLS/DASH | `yt-dlp` / Invidious API | **YES (Legal 1080p free)** |
| **Ani-One India** (`youtube.com/@AniOneIndia`) | 🟢 **LIVE (Official)** | YouTube HLS/DASH | `yt-dlp` / Invidious API | **YES (Legal 1080p free)** |
| **PureToons** (`puretoons.cc`) | 🟡 **SHIELDED (410)** | Streamwish | Embed JS unpacker | ⚠️ Requires active mirror |
| **WatchAnimeWorld** (`watchanimeworld.in`) | 🔴 **DOWN/MOVED** | Streamwish / FileLions | `Renime-API` | ⚠️ Domain dead, use DesiDubAnime |
| **AnimeSalt** (`animesalt.com`) | 🔴 **DOWN/MOVED** | DoodStream | `Renime-API` | ⚠️ Domain dead, use AnimeWorld India |
| **DeadToons** (`deadtoons.cc`) | 🔴 **DOWN/MOVED** | FileLions | Direct scraper | ⚠️ Inactive |
| **Toonstream** (`toonstream.co`) | 🔴 **DOWN/MOVED** | Multi-host | Scraper | ⚠️ Inactive |

---

## 3. Best APIs to Implement in Your App

1. **For English Sub/Dub:**
   * **HiAnime (`hianime.at`)**: Best overall. Fast 1080p HLS, soft VTT subs, easy Base64+XOR deobfuscation.
   * **GogoAnime (`anitaku.pe`)**: Largest catalog, standard AES-256 decryption.
   * **AnimeOnsen (`animeonsen.xyz`)**: Cleanest public REST API for direct 720p MP4.

2. **For Hindi Dub:**
   * **AnimeWorld India (`animeworldindia.net`)**: Live and provides direct MP4 links.
   * **DesiDubAnime (`desidubanime.me`)**: Multi-audio (Hindi, Tamil, Telugu) via Streamwish extractor.
   * **Muse India & Ani-One India**: 100% legal, zero anti-bot issues using `yt-dlp`.

# Anime Streaming UI Reference Catalog

Visual reference designs and dashboard architectures pulled into [`docs/references/images/`](file:///home/suresh/dev/projects/silvani/docs/references/images/) for building the Silvani anime streaming platform with `gpui-kit`.

---

## 1. Reference Images Overview

| Image File | Resolution | Description & Key UI Patterns |
|---|---|---|
| **[`anime_dashboard_main.webp`](file:///home/suresh/dev/projects/silvani/docs/references/images/anime_dashboard_main.webp)** | Lossless WebP | **Main Streaming Dashboard**: Minimal top bar with search input, hero spotlight banner, "Continue Watching" row, and categorized card grid. |
| **[`miru_minimal_streaming.webp`](file:///home/suresh/dev/projects/silvani/docs/references/images/miru_minimal_streaming.webp)** | 1756 x 786 | **Minimalist Desktop App**: Compact sidebar navigation, top search bar, responsive anime card grid, clean dark mode layout. |
| **[`anime_episode_details.webp`](file:///home/suresh/dev/projects/silvani/docs/references/images/anime_episode_details.webp)** | 2113 x 1273 | **Anime Details & Episode Grid**: Banner header, title & synopsis, Sub/Dub toggle, provider selector, and episode card/list selector. |
| **[`anime_search_grid.webp`](file:///home/suresh/dev/projects/silvani/docs/references/images/anime_search_grid.webp)** | 1920 x 1080 | **Multi-Provider Search Results**: Search bar with real-time results, provider tags, language badges, and quick play buttons. |
| **[`anime_player_screen.png`](file:///home/suresh/dev/projects/silvani/docs/references/images/anime_player_screen.png)** | 910 x 456 | **Player Screen Overlay**: Minimal player controls, episode drawer, resolution switcher (`1080p`, `720p`, `480p`), and caption toggle. |
| **[`mpv_anime_playback.webp`](file:///home/suresh/dev/projects/silvani/docs/references/images/mpv_anime_playback.webp)** | 1966 x 1189 | **MPV Playback Integration**: Styled anime subtitles (ASS/SSA), hardware-accelerated rendering, and dual-audio track control. |

---

## 2. Layout Breakdown for GPUI Kit Implementation

### A. Top Navigation Bar
- App Brand / Logo (`SILVANI`)
- Search input (`gpui_kit::component::input::Input`) with search icon and cleanable button
- Quick search triggers and provider filters

### B. Content Body
- **Loading State**: Animated `Skeleton` cards during scraper execution
- **Discovery Grid**: Cards with poster image, title, provider badge, and direct stream action
- **Episode Picker Modal/Drawer**: Slide-out `Sheet` or inline episode grid with Sub/Dub switcher

### C. Playback Pipeline
- Stream resolution from active providers (HiAnime, GogoAnime, AnimeOnsen, One Pace, Muse India)
- Dedicated MPV instance launched via `silvani-player` with referer headers, ASS subtitles, and IPC socket

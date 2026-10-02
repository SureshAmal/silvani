use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct AnimeTitle {
    pub english: Option<String>,
    pub romaji: Option<String>,
    pub native: Option<String>,
}

impl AnimeTitle {
    pub fn display(&self) -> &str {
        if let Some(en) = &self.english
            && !en.is_empty() {
                return en.as_str();
            }
        if let Some(ro) = &self.romaji
            && !ro.is_empty() {
                return ro.as_str();
            }
        if let Some(na) = &self.native
            && !na.is_empty() {
                return na.as_str();
            }
        "Unknown Title"
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct RelatedAnime {
    pub id: String,
    pub relation_type: String,
    pub title: AnimeTitle,
    pub format: Option<String>,
    pub episodes: Option<u32>,
    pub season: Option<String>,
    pub season_year: Option<i32>,
    pub cover_image: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct AnimeItem {
    pub id: String,
    pub mal_id: Option<i64>,
    pub title: AnimeTitle,
    pub cover_image: Option<String>,
    pub thumbnail_image: Option<String>,
    pub banner_image: Option<String>,
    pub color: Option<String>,
    pub score: Option<f32>,
    pub episodes: Option<u32>,
    pub season: Option<String>,
    pub season_year: Option<i32>,
    pub format: Option<String>,
    pub status: Option<String>,
    pub genres: Vec<String>,
    pub description: Option<String>,
    pub studio: Option<String>,
    pub next_airing_episode: Option<u32>,
    pub next_airing_at: Option<i64>,
    pub relations: Vec<RelatedAnime>,
}

impl AnimeItem {
    pub fn display_title(&self) -> &str {
        self.title.display()
    }

    pub fn season_display(&self) -> String {
        match (&self.season, self.season_year) {
            (Some(s), Some(y)) => format!("{} {}", s, y),
            (None, Some(y)) => format!("{}", y),
            (Some(s), None) => s.clone(),
            (None, None) => "".to_string(),
        }
    }

    pub fn score_display(&self) -> Option<String> {
        self.score.map(|s| format!("{:.1}", s))
    }

    pub fn clean_description(&self) -> String {
        let raw = match &self.description {
            Some(d) => d.as_str(),
            None => return "No synopsis available.".to_string(),
        };

        let s = raw
            .replace("<br>", "\n")
            .replace("<br/>", "\n")
            .replace("<br />", "\n")
            .replace("</p>", "\n\n")
            .replace("<p>", "")
            .replace("<b>", "")
            .replace("</b>", "")
            .replace("<i>", "")
            .replace("</i>", "")
            .replace("<strong>", "")
            .replace("</strong>", "")
            .replace("<em>", "")
            .replace("</em>", "")
            .replace("&quot;", "\"")
            .replace("&#039;", "'")
            .replace("&apos;", "'")
            .replace("&amp;", "&")
            .replace("&lt;", "<")
            .replace("&gt;", ">");

        let mut result = String::with_capacity(s.len());
        let mut inside_tag = false;
        for ch in s.chars() {
            if ch == '<' {
                inside_tag = true;
            } else if ch == '>' {
                inside_tag = false;
            } else if !inside_tag {
                result.push(ch);
            }
        }

        result.trim().to_string()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct DashboardData {
    pub trending: Vec<AnimeItem>,
    pub popular_season: Vec<AnimeItem>,
    pub upcoming: Vec<AnimeItem>,
    pub top_rated: Vec<AnimeItem>,
}

use silvani_scraper::Episode;
use std::collections::BTreeMap;

/// Keep provider-specific names and fractional specials, and replace only number placeholders.
pub(crate) fn enrich(episodes: &mut [Episode], titles: &BTreeMap<u32, String>) {
    for episode in episodes {
        if !episode.number.is_finite() || episode.number <= 0.0 || episode.number.fract() != 0.0 {
            continue;
        }
        let fallback = format!("Episode {}", episode.number);
        let placeholder = episode.title.as_deref().is_none_or(|title| {
            title.trim().is_empty() || title.trim().eq_ignore_ascii_case(&fallback)
        });
        if placeholder && let Some(title) = titles.get(&(episode.number as u32)) {
            episode.title = Some(title.clone());
        }
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn fills_placeholders_without_changing_ids_numbers_or_special_titles() {
        let mut episodes = vec![
            silvani_scraper::Episode {
                id: "stream-1".into(),
                number: 1.0,
                title: Some("Episode 1".into()),
            },
            silvani_scraper::Episode {
                id: "stream-2".into(),
                number: 2.0,
                title: None,
            },
            silvani_scraper::Episode {
                id: "stream-3".into(),
                number: 3.0,
                title: Some("Provider title".into()),
            },
            silvani_scraper::Episode {
                id: "special".into(),
                number: 2.5,
                title: None,
            },
        ];
        let titles = [
            (1, "A Captain".into()),
            (2, "The Voyage".into()),
            (3, "Different title".into()),
        ]
        .into();
        super::enrich(&mut episodes, &titles);
        assert_eq!(episodes[0].title.as_deref(), Some("A Captain"));
        assert_eq!(episodes[1].title.as_deref(), Some("The Voyage"));
        assert_eq!(episodes[1].id, "stream-2");
        assert_eq!(episodes[2].title.as_deref(), Some("Provider title"));
        assert!(episodes[3].title.is_none());
    }
}

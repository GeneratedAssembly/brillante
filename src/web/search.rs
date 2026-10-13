use crate::content::course::{Course, Lesson};

/// A course matched by a search, with the lessons whose titles matched.
#[derive(Debug)]
pub struct SearchHit<'a> {
    pub course: &'a Course,
    pub matched_lessons: Vec<&'a Lesson>,
    pub score: u8,
}

const LESSON_MATCH: u8 = 1;
const DESCRIPTION_MATCH: u8 = 2;
const TITLE_MATCH: u8 = 3;

/// Case-insensitive substring search over course titles/descriptions and
/// lesson titles. `visible` decides which courses the viewer may see; a
/// course it rejects is never matched, so private content cannot leak as a
/// hit. Empty (or whitespace-only) queries return no hits — the caller then
/// renders the unfiltered catalog. Stable ordering: best score first, store
/// order within a score.
pub fn search<'a>(
    courses: &'a [Course],
    q: &str,
    visible: impl Fn(&str) -> bool,
) -> Vec<SearchHit<'a>> {
    let needle = q.trim().to_lowercase();
    if needle.is_empty() {
        return Vec::new();
    }
    let mut hits: Vec<SearchHit<'a>> = courses
        .iter()
        .filter(|c| visible(&c.slug))
        .filter_map(|c| {
            let title = c.title.to_lowercase().contains(&needle);
            let description = c
                .description
                .as_deref()
                .is_some_and(|d| d.to_lowercase().contains(&needle));
            let matched_lessons: Vec<&Lesson> = c
                .lessons
                .iter()
                .filter(|l| l.title.to_lowercase().contains(&needle))
                .collect();
            let score = if title {
                TITLE_MATCH
            } else if description {
                DESCRIPTION_MATCH
            } else if !matched_lessons.is_empty() {
                LESSON_MATCH
            } else {
                return None;
            };
            Some(SearchHit {
                course: c,
                matched_lessons,
                score,
            })
        })
        .collect();
    hits.sort_by_key(|hit| std::cmp::Reverse(hit.score));
    hits
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn lesson(slug: &str, title: &str) -> Lesson {
        Lesson {
            slug: slug.into(),
            title: title.into(),
            description: None,
            order: 0.0,
            file_name: format!("{slug}.md"),
            section: None,
            body: String::new(),
        }
    }

    fn course(
        slug: &str,
        title: &str,
        description: Option<&str>,
        lessons: Vec<Lesson>,
    ) -> Course {
        Course::test_stub(
            slug.into(),
            title.into(),
            description.map(str::to_string),
            lessons,
            PathBuf::from("/nowhere"),
        )
    }

    fn all_visible() -> impl Fn(&str) -> bool {
        |_| true
    }

    #[test]
    fn empty_query_matches_nothing() {
        let courses = vec![course("a", "Algebra", None, vec![])];
        assert!(search(&courses, "", all_visible()).is_empty());
        assert!(search(&courses, "   ", all_visible()).is_empty());
    }

    #[test]
    fn matches_title_case_insensitively() {
        let courses = vec![
            course("a", "Algebra Basics", None, vec![]),
            course("b", "Calculus", Some("about ALGEBRAic limits"), vec![]),
        ];
        let hits = search(&courses, "algebra", all_visible());
        assert_eq!(hits.len(), 2);
        assert_eq!(hits[0].course.slug, "a", "title match outranks description");
        assert_eq!(hits[0].score, TITLE_MATCH);
        assert_eq!(hits[1].score, DESCRIPTION_MATCH);
    }

    #[test]
    fn lesson_title_hits_deep_link() {
        let courses = vec![course(
            "crlb",
            "Collimated Rays",
            None,
            vec![lesson("snell", "Snell's law"), lesson("rays", "Rays 101")],
        )];
        let hits = search(&courses, "snell", all_visible());
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].score, LESSON_MATCH);
        assert_eq!(hits[0].matched_lessons.len(), 1);
        assert_eq!(hits[0].matched_lessons[0].slug, "snell");
    }

    #[test]
    fn invisible_courses_never_match() {
        let courses = vec![
            course("public", "Public Optics", None, vec![lesson("p", "Public lens")]),
            course("secret", "Secret Optics", None, vec![lesson("s", "Secret lens")]),
        ];
        let hits = search(&courses, "optics", |slug| slug != "secret");
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].course.slug, "public");
        let hits = search(&courses, "secret", |slug| slug != "secret");
        assert!(hits.is_empty(), "private lesson titles must not leak");
    }

    #[test]
    fn store_order_is_stable_within_a_score() {
        let courses = vec![
            course("z", "Zeta Optics", None, vec![]),
            course("a", "Alpha Optics", None, vec![]),
        ];
        let hits = search(&courses, "optics", all_visible());
        let slugs: Vec<&str> = hits.iter().map(|h| h.course.slug.as_str()).collect();
        assert_eq!(slugs, vec!["z", "a"]);
    }
}

//! Every gallery component can be built from plain data and describes itself.

use almahrya_ui_kit::{
    Chip, DecisionCard, DecisionOption, Label, LabelStyle, ListCard, ListRow, StatusBar, Theme,
    Toggle,
};

fn option(title: &str, consequence: &str, recommended: bool) -> DecisionOption {
    DecisionOption {
        title: title.to_owned(),
        consequence: consequence.to_owned(),
        recommended,
    }
}

fn row(label: &str, value: &str) -> ListRow {
    ListRow {
        label: label.to_owned(),
        value: value.to_owned(),
    }
}

#[test]
fn gallery_components_render_without_panicking() {
    let labels = [
        Label::new("Push gate", LabelStyle::Title).label(),
        Label::new("crates/core/src/lib.rs", LabelStyle::Mono).label(),
        StatusBar::waiting("Waiting for you: one decision", "Answer").label(),
        StatusBar::agent("Agent is running the tests", "Stop").label(),
        DecisionCard::new(
            "Allow the test to write fixtures to the staging store?",
            vec![
                option(
                    "Create fixtures, remove them after the run",
                    "Writes two rows",
                    true,
                ),
                option("Skip those cases", "Two cases stay untested", false),
            ],
        )
        .label(),
        Toggle::new(true).label(),
        Toggle::new(false).label(),
        Chip::new("PROJ-123", false).label(),
        Chip::new("example-api", true).label(),
        ListCard::new(vec![
            row("Repository", "example-api"),
            row("Branch", "feat/proj-123"),
        ])
        .label(),
    ];
    for label in labels {
        assert!(!label.trim().is_empty(), "a component has an empty label");
    }
}

#[test]
fn labels_carry_the_caller_text() {
    let bar = StatusBar::waiting("Waiting for you", "Review");
    assert!(bar.label().contains("Waiting for you") && bar.label().contains("Review"));

    let card = DecisionCard::new("Pick one?", vec![option("First", "Fast", true)]);
    assert!(card.label().contains("Pick one?") && card.label().contains("First"));

    assert!(
        Chip::new("example-web", true)
            .label()
            .contains("example-web")
    );
    let list = ListCard::new(vec![row("Repository", "example-web")]);
    assert!(list.label().contains("Repository") && list.label().contains("example-web"));
    assert_eq!(Toggle::new(true).labelled("Run tests").label(), "Run tests");
}

#[test]
fn light_and_dark_themes_differ() {
    assert_ne!(Theme::dark().bg, Theme::light().bg);
    assert_ne!(Theme::dark().text, Theme::light().text);
}

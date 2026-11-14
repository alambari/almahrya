//! Every ui-kit component in the dark and the light palette, on one screen.
//!
//! `cargo run -p almahrya-ui-kit --example gallery` opens the window;
//! `-- --screenshot <path>` writes it to a PNG and exits. See docs/superpowers/gallery.

use almahrya_ui_kit::{
    Chip, ConnectionStatus, DecisionCard, DecisionOption, Gallery, GalleryColumn, GalleryItem,
    GalleryLane, GallerySection, Label, LabelStyle, ListCard, ListRow, SessionItem, StatusBar,
    Theme, Toggle, run_gallery,
};
use std::path::PathBuf;
use std::process::ExitCode;

fn main() -> ExitCode {
    let screenshot = match screenshot_path(std::env::args().skip(1)) {
        Ok(path) => path,
        Err(message) => {
            eprintln!("{message}");
            return ExitCode::from(2);
        }
    };
    run_gallery(sample(), screenshot, |outcome| match outcome {
        Ok(path) => println!("wrote {}", path.display()),
        Err(e) => {
            eprintln!("{e}");
            // The platform ends the process when the app quits, so leave with the
            // failing status now.
            std::process::exit(1);
        }
    });
    ExitCode::SUCCESS
}

fn screenshot_path(mut args: impl Iterator<Item = String>) -> Result<Option<PathBuf>, String> {
    let usage = "usage: gallery [--screenshot <path.png>]";
    match args.next().as_deref() {
        None => Ok(None),
        Some("--screenshot") => match (args.next(), args.next()) {
            (Some(path), None) => Ok(Some(PathBuf::from(path))),
            _ => Err(usage.to_owned()),
        },
        Some(_) => Err(usage.to_owned()),
    }
}

fn sample() -> Gallery {
    let option = |title: &str, consequence: &str, recommended| DecisionOption {
        title: title.to_owned(),
        consequence: consequence.to_owned(),
        recommended,
    };
    let row = |label: &str, value: &str| ListRow {
        label: label.to_owned(),
        value: value.to_owned(),
    };
    let decision = DecisionCard::new(
        "Allow the test to write fixtures to the staging store?",
        vec![
            option(
                "Create fixtures, remove them after the run",
                "Two rows exist for about a minute, then the run deletes them",
                true,
            ),
            option(
                "Skip those cases",
                "The run finishes sooner; two cases stay untested",
                false,
            ),
        ],
    )
    .recommended_tag("Recommended")
    .own_answer("Or write your own answer")
    .note("If nobody answers in 10 minutes, the recommended option is used")
    .answer_button("Answer");

    let section = |heading: &str, items: Vec<GalleryItem>| GallerySection {
        heading: heading.to_owned(),
        items,
    };
    let session = |id: &str, title: &str| SessionItem {
        id: id.to_owned(),
        title: title.to_owned(),
    };

    let narrow = GalleryLane {
        width: Some(250.),
        sections: vec![
            section(
                "Label",
                vec![GalleryItem::Labels(vec![
                    Label::new("Title: Push gate", LabelStyle::Title),
                    Label::new("Body: Two repositories are ready", LabelStyle::Body),
                    Label::new("Muted: Last run 4 minutes ago", LabelStyle::Muted),
                    Label::new("Faint: Press Enter to add", LabelStyle::Faint),
                    Label::new("crates/core/src/lib.rs", LabelStyle::Mono),
                ])],
            ),
            section(
                "Toggle and chip",
                vec![
                    GalleryItem::Toggles(vec![
                        Toggle::new(true)
                            .labelled("Run tests on save")
                            .states("On", "Off"),
                        Toggle::new(false)
                            .labelled("Push without review")
                            .states("On", "Off"),
                    ]),
                    GalleryItem::Chips(vec![
                        Chip::new("PROJ-123", false),
                        Chip::new("Merge request 42", false),
                        Chip::new("example-api", true),
                        Chip::new("example-web", true),
                    ]),
                ],
            ),
            section(
                "Sidebar",
                vec![GalleryItem::Sidebar {
                    sessions: vec![
                        session("s1", "PROJ-123 fixtures for the API tests"),
                        session("s2", "Review example-web layout"),
                        session("s3", "Upgrade the build image"),
                    ],
                    status: ConnectionStatus::Reconnecting,
                    height: 290.,
                }],
            ),
        ],
    };
    let wide = GalleryLane {
        width: None,
        sections: vec![
            section(
                "Status bar",
                vec![
                    GalleryItem::StatusBar(StatusBar::waiting(
                        "Waiting for you: one decision on PROJ-123",
                        "Answer",
                    )),
                    GalleryItem::StatusBar(StatusBar::agent(
                        "Agent is running the tests in example-api",
                        "Stop",
                    )),
                ],
            ),
            section("Decision card", vec![GalleryItem::DecisionCard(decision)]),
            section(
                "List card",
                vec![GalleryItem::ListCard(ListCard::new(vec![
                    row("Repository", "example-api"),
                    row("Target branch", "main"),
                    row("Tests", "48 passed, 0 failed"),
                ]))],
            ),
        ],
    };

    Gallery {
        title: "Almahrya gallery".to_owned(),
        width: 1440.,
        height: 900.,
        columns: vec![
            GalleryColumn {
                heading: "Dark".to_owned(),
                theme: Theme::dark(),
            },
            GalleryColumn {
                heading: "Light".to_owned(),
                theme: Theme::light(),
            },
        ],
        lanes: vec![narrow, wide],
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(list: &[&str]) -> impl Iterator<Item = String> {
        list.iter()
            .map(|s| (*s).to_owned())
            .collect::<Vec<_>>()
            .into_iter()
    }

    #[test]
    fn parses_the_screenshot_flag() {
        assert_eq!(screenshot_path(args(&[])), Ok(None));
        assert_eq!(
            screenshot_path(args(&["--screenshot", "a.png"])),
            Ok(Some(PathBuf::from("a.png")))
        );
        assert!(screenshot_path(args(&["--screenshot"])).is_err());
        assert!(screenshot_path(args(&["--other"])).is_err());
    }

    #[test]
    fn every_sample_item_has_a_label() {
        for lane in sample().lanes {
            for section in lane.sections {
                for item in section.items {
                    assert!(!item.label().is_empty());
                }
            }
        }
    }
}

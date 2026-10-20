//! The credits on your own install (skipped without `CLAW_INSTALL`): the
//! text loads from the install and wraps to the column in the game's font,
//! the parchment and its sounds are there, and the screen built from them
//! opens, scrolls a full pass and starts again as the game shows it.

use claw_acceptance::install;
use claw_assets::Install;
use claw_render::LevelGraphics;
use claw_ui::screens::ScreenAssets;
use claw_ui::screens::assets::{MENU_PICTURE, MENU_SCREENS};
use claw_ui::screens::credits::{
    self, CREDITS_PICTURE, CREDITS_SCREENS, CreditsArt, CreditsEntry, CreditsPhase, CreditsScreen,
    FontMetrics, LINE_HEIGHT, SCROLL_PX_PER_S, TEXT_WIDTH, prepare_parchment, wrap,
};

#[test]
fn install_the_credits_load_and_scroll() {
    let Some(path) = install() else { return };
    let store = Install::open(&path).unwrap().store().unwrap();
    let assets = ScreenAssets::load(store);
    // The text.
    let text = assets
        .credits_text()
        .expect("the credits text is in the install");
    let file_lines = text.lines().count();
    assert!(file_lines > 10, "{file_lines} lines");
    let (_, pal) = assets.picture(MENU_SCREENS, MENU_PICTURE).unwrap();
    let mut g = LevelGraphics::for_screens(pal, Box::new(assets.images()));
    let metrics = FontMetrics::from_art(&mut g);
    let lines = wrap(&text, &metrics, TEXT_WIDTH);
    assert!(lines.len() >= file_lines, "wrapping never joins lines");
    assert!(
        lines
            .iter()
            .all(|l| metrics.width(l) <= TEXT_WIDTH || !l.contains(' '))
    );
    // The parchment, with two palette entries for the text.
    let (img, pal) = assets
        .picture(CREDITS_SCREENS, CREDITS_PICTURE)
        .expect("the parchment is in the install");
    assert_eq!((img.width, img.height), (640, 480));
    let p = prepare_parchment(&img, &pal);
    assert_ne!(p.text_index, p.message_index);
    assert!(!p.picture.indices.contains(&p.text_index));
    assert_eq!(p.strips.len(), 40);
    // The sounds of the opening.
    for key in [credits::DELAYED_UNROLL_SOUND, credits::UNROLL_SOUND] {
        assert!(assets.sound_ms(key).is_some_and(|ms| ms > 0), "{key}");
    }
    // The screen: black, the unroll, then a whole pass of the text.
    let mut art = CreditsArt::plain(metrics);
    art.strips = p.strips.len() as i32;
    let mut s = CreditsScreen::new(CreditsEntry::AfterGame, &text, art);
    assert_eq!(s.line_count(), lines.len());
    assert_eq!(s.text_height(), lines.len() as i32 * LINE_HEIGHT);
    s.update(credits::BLACK_MS + credits::UNROLL_MS);
    assert_eq!(s.phase(), CreditsPhase::Text);
    let pass_ms = f64::from(480 + s.text_height()) * 1000.0 / SCROLL_PX_PER_S;
    eprintln!(
        "{} lines of text, {} after wrapping: a pass takes {:.0} s",
        file_lines,
        lines.len(),
        pass_ms / 1000.0
    );
    let mut t = 0.0;
    let mut drawn = false;
    while t < pass_ms + 100.0 {
        s.update(16.0);
        t += 16.0;
        drawn |= !s.scene().sprites.is_empty();
    }
    assert!(drawn, "the text shows");
    assert_eq!(s.loops(), 1, "the text starts again from the bottom");
}

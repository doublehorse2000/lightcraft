//! Permanent deletion asks from every human menu, even when ordinary delete confirmation is off.

use std::time::Duration;

use lightcraft_catalog::{PhotoId, Source};
use serde_json::json;

use super::fixtures::SyntheticLibrary;
use crate::headless::Headless;
use crate::state::Dialog;

const T: Duration = Duration::from_secs(20);

fn recently_deleted(fixture: &SyntheticLibrary, view: &str) -> (Headless, Vec<PhotoId>) {
    let mut app = fixture.app();
    let ids = app.session.visible_cloned();
    app.run("photo.delete", json!({"ids": ids})).unwrap();
    app.run("library.source", json!({"kind": "recentlyDeleted"})).unwrap();
    app.run("library.select", json!({"ids": [ids[0]]})).unwrap();
    app.ui.settings.confirm_delete = false;
    let mut h = Headless::new(app, [1400.0, 900.0], 1.0);
    let r = h.request("ui.set", json!({"view": view, "filmstrip": true}), T);
    assert_eq!(r["ok"], true, "{r}");
    h.settle(T);
    (h, ids)
}

fn click(h: &mut Headless, id: &str) {
    let r = h.request("ui.clickWidget", json!({"id": id}), T);
    assert_eq!(r["ok"], true, "{r}");
    h.step();
    h.step();
}

fn ask(h: &mut Headless) {
    crate::menubar::run_item(&mut h.app, "photo.deletePermanently", json!({})).unwrap();
    for _ in 0..4 {
        h.step();
    }
    assert!(matches!(h.app.ui.dialog, Some(Dialog::ConfirmDeletePermanently { .. })));
}

#[test]
fn the_photo_menu_asks_for_one_or_several_and_only_confirm_removes_them() {
    for count in [1, 3] {
        let fixture = SyntheticLibrary::with_photos(&format!("permanent-menu-{count}"), 4);
        let (mut h, ids) = recently_deleted(&fixture, "photoGrid");
        let targets = &ids[..count];
        h.app.run("library.select", json!({"ids": targets})).unwrap();
        let originals: Vec<_> = targets
            .iter()
            .map(|id| match &h.app.session.catalog.photo(*id).unwrap().source {
                Source::File { path } => (path.clone(), std::fs::read(path).unwrap()),
                _ => panic!("the fixture has real files"),
            })
            .collect();
        ask(&mut h);
        assert!(matches!(&h.app.ui.dialog, Some(Dialog::ConfirmDeletePermanently { ids, .. }) if ids.len() == count));
        assert!(ids.iter().all(|id| h.app.session.catalog.photo(*id).is_some()), "nothing removed before the answer");
        click(&mut h, "button:dialogCancel");
        assert!(h.app.ui.dialog.is_none());
        assert!(ids.iter().all(|id| h.app.session.catalog.photo(*id).is_some()), "Cancel keeps every photo");

        ask(&mut h);
        click(&mut h, "button:dialogOk");
        assert!(h.app.ui.dialog.is_none());
        assert!(targets.iter().all(|id| h.app.session.catalog.photo(*id).is_none()));
        assert!(ids[count..].iter().all(|id| h.app.session.catalog.photo(*id).is_some()));
        for (path, bytes) in &originals {
            assert_eq!(std::fs::read(path).unwrap(), *bytes, "original files stay unchanged");
        }
        h.app.run("edit.undo", json!({})).unwrap();
        assert!(targets.iter().all(|id| h.app.session.catalog.photo(*id).is_some_and(|p| p.deleted)), "Undo restores Recently Deleted");
    }
}

#[test]
fn both_grids_and_the_filmstrip_context_menu_ask_before_permanent_deletion() {
    for view in ["photoGrid", "squareGrid", "detail"] {
        let fixture = SyntheticLibrary::with_photos(&format!("permanent-context-{view}"), 3);
        let (mut h, ids) = recently_deleted(&fixture, view);
        let targets = &ids[..2];
        h.app.run("library.select", json!({"ids": targets})).unwrap();
        h.step();
        let cell = format!("{}:{}", if view == "detail" { "film" } else { "thumb" }, ids[0].0);
        let at = h.app.widgets.iter().find(|(id, _)| *id == cell).unwrap().1.center();
        let r = h.request("ui.click", json!({"x": at.x, "y": at.y, "button": "right"}), T);
        assert_eq!(r["ok"], true, "{r}");
        h.step();
        h.step();
        click(&mut h, "button:contextDeletePermanently");
        assert!(matches!(&h.app.ui.dialog, Some(Dialog::ConfirmDeletePermanently { ids, .. }) if ids.len() == 2), "{view}");
        assert!(targets.iter().all(|id| h.app.session.catalog.photo(*id).is_some()));
        click(&mut h, "button:dialogOk");
        assert!(targets.iter().all(|id| h.app.session.catalog.photo(*id).is_none()), "{view}");
        assert!(h.app.session.catalog.photo(ids[2]).is_some());
    }
}

#[test]
fn escape_and_return_cancel_instead_of_permanently_deleting() {
    for key in ["escape", "enter"] {
        let fixture = SyntheticLibrary::new(&format!("permanent-key-{key}"));
        let (mut h, ids) = recently_deleted(&fixture, "photoGrid");
        ask(&mut h);
        let r = h.request("ui.key", json!({"key": key}), T);
        assert_eq!(r["ok"], true, "{r}");
        assert!(h.app.ui.dialog.is_none(), "{key} cancels");
        assert!(h.app.session.catalog.photo(ids[0]).is_some(), "{key} keeps the photo");
    }
}

#[test]
fn confirmation_keeps_its_targets_when_the_selection_changes() {
    let fixture = SyntheticLibrary::with_photos("permanent-selection", 3);
    let (mut h, ids) = recently_deleted(&fixture, "photoGrid");
    ask(&mut h);
    h.app.run("library.select", json!({"ids": [ids[1]]})).unwrap();
    let r = h.request("ui.dialog.confirm", json!({}), T);
    assert_eq!(r["ok"], true, "{r}");
    assert!(h.app.session.catalog.photo(ids[0]).is_none());
    assert!(ids[1..].iter().all(|id| h.app.session.catalog.photo(*id).is_some()));
}

#[test]
fn explicit_targets_are_preserved_and_bad_targets_never_use_the_selection() {
    let fixture = SyntheticLibrary::with_photos("permanent-explicit", 3);
    let (mut h, ids) = recently_deleted(&fixture, "photoGrid");
    let r = crate::menubar::run_item(&mut h.app, "photo.deletePermanently", json!({"ids": "bad"}));
    assert!(r.is_err());
    assert!(h.app.ui.dialog.is_none());
    crate::menubar::run_item(&mut h.app, "photo.deletePermanently", json!({"ids": [ids[1], ids[1]]})).unwrap();
    assert!(matches!(&h.app.ui.dialog, Some(Dialog::ConfirmDeletePermanently { ids, .. }) if ids.len() == 1));
    let r = h.request("ui.dialog.confirm", json!({}), T);
    assert_eq!(r["ok"], true, "{r}");
    assert!(h.app.session.catalog.photo(ids[0]).is_some());
    assert!(h.app.session.catalog.photo(ids[1]).is_none());
    assert!(h.app.session.catalog.photo(ids[2]).is_some());
}

#[test]
fn a_confirmation_from_another_library_cannot_delete_a_reused_photo_id() {
    let fixture = SyntheticLibrary::new("permanent-library");
    let (mut h, ids) = recently_deleted(&fixture, "photoGrid");
    ask(&mut h);
    h.app.session = fixture.app().session;
    assert!(h.app.session.catalog.photo(ids[0]).is_some(), "the new library reuses the id");
    let r = h.request("ui.dialog.confirm", json!({}), T);
    assert_eq!(r["ok"], false, "{r}");
    assert!(r["error"].as_str().unwrap().contains("Another library was opened"), "{r}");
    assert!(h.app.session.catalog.photo(ids[0]).is_some(), "the new library stays untouched");
}

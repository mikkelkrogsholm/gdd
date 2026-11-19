use super::{Ui, Viewer, egui, projection::projection_details};
use serde_json::{Value as Json, json};
use std::{
    io::{self, Write},
    path::{Path, PathBuf},
};

#[derive(Default)]
pub(super) struct SaveDialog {
    destination: String,
    error: Option<String>,
    focused: bool,
}

impl Viewer {
    pub(super) fn open_save_dialog(&mut self) {
        self.saved_path = None;
        self.save_dialog.get_or_insert_with(SaveDialog::default);
    }

    pub(super) fn save_button(&mut self, ui: &mut Ui, shortcut: &egui::KeyboardShortcut) {
        if ui
            .button("Save view state…")
            .on_hover_text(ui.ctx().format_shortcut(shortcut))
            .clicked()
        {
            self.open_save_dialog();
        }
    }

    pub(super) fn show_save_dialog(&mut self, ctx: &egui::Context) {
        let Some(dialog) = &mut self.save_dialog else {
            return;
        };
        let modal = egui::Modal::new(egui::Id::unique("save-view-state")).show(ctx, |ui| {
            ui.set_width((ctx.content_rect().width() - 40.0).min(420.0));
            ui.heading("Save view state");
            let label = ui.label("New file path");
            let edit = ui
                .add(
                    egui::TextEdit::singleline(&mut dialog.destination)
                        .desired_width(f32::INFINITY)
                        .hint_text("view-state.json"),
                )
                .labelled_by(label.id);
            if !dialog.focused {
                edit.request_focus();
                dialog.focused = true;
            }
            if edit.changed() {
                dialog.error = None;
            }
            if let Some(error) = &dialog.error {
                ui.colored_label(ui.visuals().error_fg_color, error);
            }
            let enter = edit.lost_focus() && ui.input(|input| input.key_pressed(egui::Key::Enter));
            ui.horizontal(|ui| {
                let save = ui
                    .add_enabled(!dialog.destination.is_empty(), egui::Button::new("Save"))
                    .clicked();
                if ui.button("Cancel").clicked() {
                    ui.close();
                }
                (enter || save) && !dialog.destination.is_empty()
            })
            .inner
        });
        if modal.should_close() {
            self.save_dialog = None;
        } else if modal.inner {
            let details = projection_details(
                &self.graph,
                &self.filters,
                &self.visible,
                self.folded.as_deref(),
            );
            match save_projection(Path::new(&dialog.destination), details) {
                Ok(path) => {
                    self.saved_path = Some(path);
                    self.save_dialog = None;
                }
                Err(error) => {
                    dialog.error = Some(format!("Could not save view state: {error}"));
                }
            }
        }
    }
}

fn save_projection(destination: &Path, mut details: Json) -> io::Result<PathBuf> {
    if !details["spec_revision"].is_string() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "The model has no resolved snapshot revision.",
        ));
    }
    details["format"] = json!("gdd-native-view-state/0.1");
    details["generator"] = json!({
        "name": "gdd-viewer",
        "version": env!("CARGO_PKG_VERSION"),
        "profile": gdd::view::PROFILE,
    });
    let path = std::path::absolute(destination)?;
    let directory = path.parent().ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            "Choose a new file in a directory.",
        )
    })?;
    let mut staged = tempfile::NamedTempFile::new_in(directory)?;
    {
        let mut writer = io::BufWriter::new(staged.as_file_mut());
        serde_json::to_writer_pretty(&mut writer, &details)?;
        writer.write_all(b"\n")?;
        writer.flush()?;
    }
    staged.persist_noclobber(&path).map_err(|error| {
        if error.error.kind() == io::ErrorKind::AlreadyExists {
            io::Error::new(
                io::ErrorKind::AlreadyExists,
                "The destination already exists. Choose a new file path.",
            )
        } else {
            error.error
        }
    })?;
    Ok(path)
}

use crate::app::InputCommand;
use crate::app::command::{move_next, move_prev};
use crate::app::ui::header::show_header_row;
use crate::app::ui::theme::Theme;
use crate::app::ui::widgets::smooth_scroll_area;
use crate::i18n::{I18n, arg_string};
use crate::settings::RegionBypass;
use crate::{App, AppCommand, AppState, Locale};
use anyhow::Result;
use fluent_bundle::FluentArgs;

#[derive(Clone, PartialEq)]
pub enum Command {
    ToggleLocaleExpanded,
    SetLocale(Locale),
    SetSwapShouldersAndTriggers { title_id: String, enabled: bool },
    SetRearTouchEnabled { title_id: String, enabled: bool },
    SetFrontTouchAuxiliaryButtons { title_id: String, enabled: bool },
    SetShowStreamDebugInfo(bool),
    SetRegionBypass(RegionBypass),
    SetPreferredServer(Option<String>),
}

#[derive(Clone)]
enum SettingsRow {
    LocaleToggle,
    LocaleOption(Locale),
    GameSwap { title_id: String, enabled: bool },
    GameRearTouch { title_id: String, enabled: bool },
    GameFrontTouchAuxiliary { title_id: String, enabled: bool },
    StreamDebug(bool),
    RegionBypass,
    Server,
    Back,
}

/// `None` (automatic) followed by every server offered at sign-in. A saved preference that is
/// not in the current list is kept so cycling never silently drops it.
fn server_options(app: &App) -> Vec<Option<String>> {
    let mut options = vec![None];
    options.extend(app.service.cloud_regions.iter().cloned().map(Some));
    if let Some(saved) = &app.settings.preferred_server {
        if !options
            .iter()
            .flatten()
            .any(|name| name.eq_ignore_ascii_case(saved))
        {
            options.push(Some(saved.clone()));
        }
    }
    options
}

fn cycle_server(app: &App, forward: bool) -> Option<String> {
    let options = server_options(app);
    let current = options
        .iter()
        .position(|option| match (option, &app.settings.preferred_server) {
            (None, None) => true,
            (Some(a), Some(b)) => a.eq_ignore_ascii_case(b),
            _ => false,
        })
        .unwrap_or(0);
    let next = if forward {
        move_next(current, options.len())
    } else {
        move_prev(current, options.len())
    };
    options[next].clone()
}

fn region_bypass_label(i18n: &I18n, bypass: RegionBypass) -> String {
    let value = match bypass.code() {
        None => i18n.text("settings-region-off"),
        Some(code) => format!("{} ({code})", i18n.text(country_key(bypass))),
    };
    format!("{}:  < {value} >", i18n.text("settings-region-bypass"))
}

fn country_key(bypass: RegionBypass) -> &'static str {
    match bypass {
        RegionBypass::Off => "settings-region-off",
        RegionBypass::Brazil => "country-br",
        RegionBypass::UnitedStates => "country-us",
        RegionBypass::Japan => "country-jp",
        RegionBypass::Korea => "country-kr",
        RegionBypass::Poland => "country-pl",
    }
}

fn server_label(i18n: &I18n, preferred: Option<&str>) -> String {
    let value = preferred
        .map(str::to_owned)
        .unwrap_or_else(|| i18n.text("settings-server-auto"));
    format!("{}:  < {value} >", i18n.text("settings-server"))
}

fn settings_rows(app: &App) -> Vec<SettingsRow> {
    let AppState::Settings {
        title_id,
        locale_expanded,
        ..
    } = &app.state
    else {
        return Vec::new();
    };
    let mut rows = Vec::new();
    rows.push(SettingsRow::LocaleToggle);
    if *locale_expanded {
        rows.extend(Locale::ALL.iter().copied().map(SettingsRow::LocaleOption));
    }
    rows.push(SettingsRow::RegionBypass);
    rows.push(SettingsRow::Server);
    if let Some(title_id) = title_id.clone() {
        let enabled = app
            .settings
            .game_profile(&title_id)
            .is_some_and(|profile| profile.swap_shoulders_and_triggers);
        rows.push(SettingsRow::GameSwap {
            title_id: title_id.clone(),
            enabled,
        });
        let enabled = app
            .settings
            .game_profile(&title_id)
            .is_none_or(|profile| profile.rear_touch_enabled);
        rows.push(SettingsRow::GameRearTouch {
            title_id: title_id.clone(),
            enabled,
        });
        let enabled = app
            .settings
            .game_profile(&title_id)
            .is_some_and(|profile| profile.front_touch_auxiliary_buttons);
        rows.push(SettingsRow::GameFrontTouchAuxiliary { title_id, enabled });
    }
    rows.push(SettingsRow::StreamDebug(
        app.settings.show_stream_debug_info,
    ));
    rows.push(SettingsRow::Back);
    rows
}

/// Editable settings, persisted immediately on each change. Every interactive row is a
/// `SelectableLabel` (`focus_row`) addressed by one flat index, so
/// D-pad + Confirm can reach every row, not just touch.
pub(crate) fn show(ctx: &egui::Context, app: &App, commands: &mut Vec<AppCommand>) {
    let theme = Theme::dark();
    let i18n = I18n::new(app.settings.locale);
    let AppState::Settings {
        title_id,
        locale_expanded,
        selected,
        ..
    } = &app.state
    else {
        return;
    };
    let title_id = title_id.clone();
    let locale_expanded = *locale_expanded;
    let selected_index = *selected;
    let mut frame = egui::Frame::central_panel(&ctx.style());
    frame.fill = theme.background;
    egui::CentralPanel::default().frame(frame).show(ctx, |ui| {
        show_header_row(ui, app, theme, &i18n, None);
        ui.separator();
        let locale = app.settings.locale;
        let mut row_index = 0;

        ui.visuals_mut().selection.bg_fill = theme.accent;
        ui.visuals_mut().selection.stroke.color = egui::Color32::WHITE;

        smooth_scroll_area(ui, "settings_scroll", |ui| {
            ui.heading(egui::RichText::new(i18n.text("settings-locale")).color(theme.text_bright));
            ui.vertical(|ui| {
                let toggle_label = format!(
                    "{}  {}",
                    locale.label(),
                    if locale_expanded { "v" } else { ">" }
                );
                if focus_row(ui, selected_index == row_index, toggle_label) {
                    commands.push(Command::ToggleLocaleExpanded.into());
                }
                row_index += 1;

                if locale_expanded {
                    for option in Locale::ALL.iter().copied() {
                        let mut text = egui::RichText::new(option.label());
                        if locale == option {
                            text = text.color(theme.accent).strong();
                        }
                        if focus_row(ui, selected_index == row_index, text) {
                            commands.push(Command::SetLocale(option).into());
                        }
                        row_index += 1;
                    }
                }

                ui.add_space(14.0);
                ui.separator();
                ui.heading(
                    egui::RichText::new(i18n.text("settings-region")).color(theme.text_bright),
                );
                if focus_row(
                    ui,
                    selected_index == row_index,
                    region_bypass_label(&i18n, app.settings.region_bypass),
                ) {
                    commands
                        .push(Command::SetRegionBypass(app.settings.region_bypass.next()).into());
                }
                row_index += 1;
                if focus_row(
                    ui,
                    selected_index == row_index,
                    server_label(&i18n, app.settings.preferred_server.as_deref()),
                ) {
                    commands.push(Command::SetPreferredServer(cycle_server(app, true)).into());
                }
                row_index += 1;
                if let Some(current) = &app.service.cloud_region {
                    let mut args = FluentArgs::new();
                    args.set("server", arg_string(current.as_str()));
                    ui.colored_label(theme.text, i18n.text_with("settings-server-current", args));
                }
                ui.colored_label(theme.text, i18n.text("settings-region-hint"));

                if let Some(title_id) = title_id.clone() {
                    let swap_shoulders_and_triggers = app
                        .settings
                        .game_profile(&title_id)
                        .is_some_and(|profile| profile.swap_shoulders_and_triggers);

                    ui.add_space(14.0);
                    ui.separator();
                    ui.heading(
                        egui::RichText::new(i18n.text("settings-game")).color(theme.text_bright),
                    );
                    ui.colored_label(theme.text_bright, app.service.title_name_or_id(&title_id));

                    if checkbox_row(
                        ui,
                        selected_index == row_index,
                        swap_shoulders_and_triggers,
                        i18n.text("settings-swap-shoulders-triggers"),
                    ) {
                        commands.push(
                            Command::SetSwapShouldersAndTriggers {
                                title_id: title_id.clone(),
                                enabled: !swap_shoulders_and_triggers,
                            }
                            .into(),
                        );
                    }
                    row_index += 1;

                    let rear_touch_enabled = app
                        .settings
                        .game_profile(&title_id)
                        .is_none_or(|profile| profile.rear_touch_enabled);
                    if checkbox_row(
                        ui,
                        selected_index == row_index,
                        rear_touch_enabled,
                        i18n.text("settings-rear-touch-enabled"),
                    ) {
                        commands.push(
                            Command::SetRearTouchEnabled {
                                title_id: title_id.clone(),
                                enabled: !rear_touch_enabled,
                            }
                            .into(),
                        );
                    }
                    row_index += 1;

                    let front_touch_auxiliary_buttons = app
                        .settings
                        .game_profile(&title_id)
                        .is_some_and(|profile| profile.front_touch_auxiliary_buttons);
                    if checkbox_row(
                        ui,
                        selected_index == row_index,
                        front_touch_auxiliary_buttons,
                        i18n.text("settings-front-touch-auxiliary-buttons"),
                    ) {
                        commands.push(
                            Command::SetFrontTouchAuxiliaryButtons {
                                title_id,
                                enabled: !front_touch_auxiliary_buttons,
                            }
                            .into(),
                        );
                    }
                    row_index += 1;
                }

                ui.add_space(14.0);
                ui.separator();
                ui.colored_label(
                    theme.text,
                    host_text(
                        &i18n,
                        "settings-cloud-host",
                        &app.service.api.config.cloud.host,
                    ),
                );
                ui.colored_label(
                    theme.text,
                    host_text(
                        &i18n,
                        "settings-home-host",
                        &app.service.api.config.home.host,
                    ),
                );

                ui.add_space(14.0);
                ui.separator();
                if checkbox_row(
                    ui,
                    selected_index == row_index,
                    app.settings.show_stream_debug_info,
                    i18n.text("settings-stream-debug-info"),
                ) {
                    commands.push(
                        Command::SetShowStreamDebugInfo(!app.settings.show_stream_debug_info)
                            .into(),
                    );
                }
                row_index += 1;

                ui.add_space(14.0);
                ui.separator();
                if focus_row(ui, selected_index == row_index, i18n.text("action-back")) {
                    commands.push(InputCommand::Back.into());
                }
            });
        });
    });
}

fn focus_row(ui: &mut egui::Ui, selected: bool, text: impl Into<egui::WidgetText>) -> bool {
    ui.add_sized(
        egui::vec2(ui.available_width(), 28.0),
        egui::SelectableLabel::new(selected, text),
    )
    .clicked()
}

/// A full-width focusable row with a drawn checkbox and its label side by side.
fn checkbox_row(ui: &mut egui::Ui, selected: bool, checked: bool, label: String) -> bool {
    const ROW_HEIGHT: f32 = 32.0;
    const BOX_SIZE: f32 = 18.0;

    let (rect, response) = ui.allocate_exact_size(
        egui::vec2(ui.available_width(), ROW_HEIGHT),
        egui::Sense::click(),
    );

    if ui.is_rect_visible(rect) {
        let selection = ui.visuals().selection;
        let row_fill = if selected {
            selection.bg_fill
        } else if response.hovered() {
            ui.visuals().widgets.hovered.bg_fill
        } else {
            egui::Color32::TRANSPARENT
        };
        ui.painter().rect_filled(rect, 4.0, row_fill);

        let box_rect = egui::Rect::from_center_size(
            egui::pos2(rect.left() + 8.0 + BOX_SIZE / 2.0, rect.center().y),
            egui::vec2(BOX_SIZE, BOX_SIZE),
        );
        let accent = selection.bg_fill;
        let box_fill = if checked {
            if selected {
                egui::Color32::WHITE
            } else {
                accent
            }
        } else {
            egui::Color32::TRANSPARENT
        };
        let outline = if selected {
            egui::Color32::WHITE
        } else {
            ui.visuals().widgets.inactive.fg_stroke.color
        };
        ui.painter().rect_filled(box_rect, 3.0, box_fill);
        ui.painter().rect_stroke(
            box_rect,
            3.0,
            egui::Stroke::new(1.5, outline),
            egui::StrokeKind::Inside,
        );

        if checked {
            let check_color = if selected {
                accent
            } else {
                egui::Color32::WHITE
            };
            let check_stroke = egui::Stroke::new(2.5, check_color);
            let first = egui::pos2(box_rect.left() + 3.5, box_rect.center().y);
            let middle = egui::pos2(box_rect.left() + 7.5, box_rect.bottom() - 4.0);
            let last = egui::pos2(box_rect.right() - 3.0, box_rect.top() + 4.0);
            ui.painter().line_segment([first, middle], check_stroke);
            ui.painter().line_segment([middle, last], check_stroke);
        }

        let text_color = if selected {
            egui::Color32::WHITE
        } else {
            ui.visuals().text_color()
        };
        ui.painter().text(
            egui::pos2(box_rect.right() + 10.0, rect.center().y),
            egui::Align2::LEFT_CENTER,
            label,
            egui::FontId::proportional(16.0),
            text_color,
        );
    }

    response.clicked()
}

fn host_text(i18n: &I18n, id: &'static str, host: &str) -> String {
    let mut args = FluentArgs::new();
    args.set("host", arg_string(host));
    i18n.text_with(id, args)
}

impl App {
    pub(crate) fn handle_settings_input(&mut self, command: InputCommand) -> Result<()> {
        let rows = settings_rows(self);
        match command {
            InputCommand::MoveUp => {
                if let AppState::Settings { selected, .. } = &mut self.state {
                    *selected = move_prev(*selected, rows.len());
                }
            }
            InputCommand::MoveDown => {
                if let AppState::Settings { selected, .. } = &mut self.state {
                    *selected = move_next(*selected, rows.len());
                }
            }
            InputCommand::MoveLeft | InputCommand::MoveRight => {
                let forward = matches!(command, InputCommand::MoveRight);
                let selected = match &self.state {
                    AppState::Settings { selected, .. } => *selected,
                    _ => return Ok(()),
                };
                match rows.get(selected) {
                    Some(SettingsRow::RegionBypass) => {
                        let current = self.settings.region_bypass;
                        let next = if forward {
                            current.next()
                        } else {
                            current.prev()
                        };
                        self.handle_settings_command(Command::SetRegionBypass(next))?;
                    }
                    Some(SettingsRow::Server) => {
                        let next = cycle_server(self, forward);
                        self.handle_settings_command(Command::SetPreferredServer(next))?;
                    }
                    _ => {}
                }
            }
            InputCommand::Confirm => self.confirm_settings_row(&rows)?,
            InputCommand::Back => self.leave_settings(),
        }

        Ok(())
    }

    /// Activates the selected row from D-pad input.
    fn confirm_settings_row(&mut self, rows: &[SettingsRow]) -> Result<()> {
        let AppState::Settings { selected, .. } = &self.state else {
            return Ok(());
        };
        let Some(row) = rows.get(*selected) else {
            self.leave_settings();
            return Ok(());
        };

        match row {
            SettingsRow::LocaleToggle => {
                return self.handle_settings_command(Command::ToggleLocaleExpanded);
            }
            SettingsRow::LocaleOption(locale) => {
                return self.handle_settings_command(Command::SetLocale(*locale));
            }
            SettingsRow::GameSwap { title_id, enabled } => {
                return self.handle_settings_command(Command::SetSwapShouldersAndTriggers {
                    title_id: title_id.clone(),
                    enabled: !enabled,
                });
            }
            SettingsRow::GameRearTouch { title_id, enabled } => {
                return self.handle_settings_command(Command::SetRearTouchEnabled {
                    title_id: title_id.clone(),
                    enabled: !enabled,
                });
            }
            SettingsRow::GameFrontTouchAuxiliary { title_id, enabled } => {
                return self.handle_settings_command(Command::SetFrontTouchAuxiliaryButtons {
                    title_id: title_id.clone(),
                    enabled: !enabled,
                });
            }
            SettingsRow::StreamDebug(enabled) => {
                return self.handle_settings_command(Command::SetShowStreamDebugInfo(!enabled));
            }
            SettingsRow::RegionBypass => {
                let next = self.settings.region_bypass.next();
                return self.handle_settings_command(Command::SetRegionBypass(next));
            }
            SettingsRow::Server => {
                let next = cycle_server(self, true);
                return self.handle_settings_command(Command::SetPreferredServer(next));
            }
            SettingsRow::Back => {}
        }

        self.leave_settings();
        Ok(())
    }

    fn leave_settings(&mut self) {
        let state = std::mem::replace(&mut self.state, AppState::TitleList { selected: 0 });
        if let AppState::Settings { return_to, .. } = state {
            // A new region needs a new streaming token. Never interrupt a live stream for it;
            // the change then applies the next time Settings is left outside a stream. When not
            // signed in yet, the next sign-in picks the new settings up on its own.
            let signed_in = self.service.cloud_region.is_some();
            if self.region_changed && signed_in && !return_to.keeps_stream_alive() {
                self.region_changed = false;
                self.load_credentials();
            } else {
                self.set_state(*return_to);
            }
        } else {
            self.state = state;
        }
    }

    pub(crate) fn handle_settings_command(&mut self, command: Command) -> Result<()> {
        match command {
            Command::ToggleLocaleExpanded => {
                let expanded = matches!(
                    &self.state,
                    AppState::Settings {
                        locale_expanded: false,
                        ..
                    }
                );
                let selected = if expanded {
                    let current = Locale::ALL
                        .iter()
                        .position(|&locale| locale == self.settings.locale)
                        .unwrap_or(0);
                    1 + current
                } else {
                    0
                };
                if let AppState::Settings {
                    locale_expanded,
                    selected: current,
                    ..
                } = &mut self.state
                {
                    *locale_expanded = expanded;
                    *current = selected;
                }
            }
            Command::SetLocale(locale) => {
                self.set_locale(locale);
                if let AppState::Settings {
                    locale_expanded,
                    selected,
                    ..
                } = &mut self.state
                {
                    *locale_expanded = false;
                    *selected = 0;
                }
            }
            Command::SetSwapShouldersAndTriggers { title_id, enabled } => {
                self.settings
                    .set_swap_shoulders_and_triggers(title_id, enabled);
                self.settings.save();
            }
            Command::SetRearTouchEnabled { title_id, enabled } => {
                self.settings.set_rear_touch_enabled(title_id, enabled);
                self.settings.save();
            }
            Command::SetFrontTouchAuxiliaryButtons { title_id, enabled } => {
                self.settings
                    .set_front_touch_auxiliary_buttons(title_id, enabled);
                self.settings.save();
            }
            Command::SetShowStreamDebugInfo(enabled) => {
                self.settings.show_stream_debug_info = enabled;
                self.settings.save();
            }
            Command::SetRegionBypass(bypass) => {
                if self.settings.region_bypass != bypass {
                    self.settings.region_bypass = bypass;
                    self.settings.apply_region_config();
                    self.settings.save();
                    self.region_changed = true;
                }
            }
            Command::SetPreferredServer(server) => {
                if self.settings.preferred_server != server {
                    self.settings.preferred_server = server;
                    self.settings.apply_region_config();
                    self.settings.save();
                    self.region_changed = true;
                }
            }
        }

        Ok(())
    }

    pub(crate) fn set_locale(&mut self, locale: Locale) {
        if self.settings.locale == locale {
            return;
        }

        self.settings.locale = locale;
        self.service.api.config.locale = locale.as_str().to_owned();
        self.invalidate_catalog_for_locale_change();
        self.settings.save();
    }
}

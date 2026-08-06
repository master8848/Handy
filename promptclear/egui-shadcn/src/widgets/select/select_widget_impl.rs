//! Widget trait implementation for Select and SelectValue.

impl<T: Clone + std::fmt::Display + PartialEq + 'static> egui::Widget
    for super::select::Select<'_, T>
{
    fn ui(self, ui: &mut egui::Ui) -> egui::Response {
        let theme = crate::theme::shadcn_theme_ext::ShadcnThemeExt::shadcn_theme(ui.ctx());
        let style = super::select_style::resolve_select_style(&theme);

        let height: f32 = 32.0;
        let h_padding: f32 = 10.0;
        let chevron_width: f32 = 20.0;
        let width = self.width.unwrap_or(ui.available_width().min(200.0));
        let desired = egui::vec2(width, height);

        let (rect, response) = ui.allocate_exact_size(desired, egui::Sense::click());
        let popup_id = response.id.with("popup");

        if ui.is_rect_visible(rect) {
            let painter = ui.painter();
            let cr = egui::CornerRadius::same(style.corner_radius.round() as u8);
            let pressed = response.is_pointer_button_down_on();
            let trigger_bg = if pressed {
                crate::paint::interpolate_color::interpolate_color(
                    style.trigger_bg,
                    theme.accent,
                    0.85,
                )
            } else if response.hovered() {
                theme.accent
            } else {
                style.trigger_bg
            };
            let trigger_border = if response.hovered() || pressed {
                theme.ring
            } else {
                style.trigger_border
            };

            painter.rect_filled(rect, cr, trigger_bg);
            painter.rect_stroke(
                rect,
                cr,
                egui::Stroke::new(1.0, trigger_border),
                egui::epaint::StrokeKind::Inside,
            );

            // Display text — use override if provided
            let display_text = if let Some(ref override_text) = self.selected_text_override {
                override_text.clone()
            } else {
                match &self.selected {
                    Some(val) => val.to_string(),
                    None => self.placeholder.clone(),
                }
            };

            let text_color = if self.selected.is_some() || self.selected_text_override.is_some() {
                style.trigger_text
            } else {
                theme.muted_foreground
            };

            let galley =
                painter.layout_no_wrap(display_text, egui::FontId::proportional(14.0), text_color);
            let text_pos = egui::pos2(
                rect.min.x + h_padding,
                rect.center().y - galley.size().y / 2.0,
            );
            painter.galley(text_pos, galley, text_color);

            let icon_size: f32 = 14.0;
            let chevron_rect = egui::Rect::from_center_size(
                egui::pos2(rect.max.x - chevron_width / 2.0 - 2.0, rect.center().y),
                egui::vec2(icon_size, icon_size),
            );
            crate::icons::paint_icon::paint_icon(
                painter,
                chevron_rect,
                &crate::icons::lucide_icon::LucideIcon::ChevronDown,
                theme.muted_foreground,
            );

            if response.has_focus() {
                crate::paint::paint_focus_ring::paint_focus_ring(
                    painter,
                    rect,
                    style.corner_radius,
                    theme.ring,
                );
            }
        }

        if response.hovered() {
            ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
        }

        // The popup id derives from the trigger's auto-id. If the widget's
        // position in the UI tree ever changes (window/tab closed, layout
        // reordered), an old popup id can linger in `Memory::popups` for a
        // frame. `Memory::end_pass` prunes popups that were not re-asserted
        // with `keep_popup_open`, but if the popup was merely *rendered* in a
        // different place (e.g. the settings window reopened), the stale entry
        // survives. A stale entry would turn the first trigger click into a
        // "toggle-closed" no-op, so purge it explicitly: a popup that is open
        // in memory but whose area was not drawn last frame is stale.
        if egui::Popup::is_id_open(ui.ctx(), popup_id) && ui.ctx().read_response(popup_id).is_none()
        {
            egui::Popup::close_id(ui.ctx(), popup_id);
        }

        let toggle_cmd = if response.clicked() {
            // Always open on a trigger click instead of toggling: a stale (or
            // just-orphaned) entry in popup memory must never turn the first
            // click into a close. The popup still closes on outside clicks,
            // Esc, and item selection (see below).
            Some(egui::SetOpenCommand::Bool(true))
        } else {
            None
        };

        let popup_cr = style.corner_radius.round() as u8;
        let popup = egui::Popup::new(popup_id, ui.ctx().clone(), &response, ui.layer_id())
            .open_memory(toggle_cmd)
            .close_behavior(egui::PopupCloseBehavior::CloseOnClickOutside)
            .frame(
                egui::Frame::NONE
                    .fill(style.popover_bg)
                    .inner_margin(egui::Margin::same(4))
                    .corner_radius(egui::CornerRadius::same(popup_cr))
                    .stroke(egui::Stroke::new(1.0, style.popover_border))
                    .shadow(egui::Shadow {
                        offset: [0, 4],
                        blur: 12,
                        spread: 0,
                        color: egui::Color32::from_black_alpha(8),
                    }),
            );

        popup.show(|ui: &mut egui::Ui| {
            let popup_width = width.max(144.0);
            ui.set_min_width(popup_width);
            ui.set_max_width(popup_width);
            let check_icon_size: f32 = 12.0;

            for option in self.options {
                let is_selected = self.selected.as_ref() == Some(option);
                let label = option.to_string();

                let galley = ui.painter().layout_no_wrap(
                    label.clone(),
                    egui::FontId::proportional(14.0),
                    style.item_text,
                );

                let item_height = galley.size().y + 8.0;
                let item_desired = egui::vec2(popup_width, item_height);
                let (item_rect, item_response) =
                    ui.allocate_exact_size(item_desired, egui::Sense::click());

                if ui.is_rect_visible(item_rect) {
                    let item_cr = egui::CornerRadius::same(
                        (style.corner_radius - 2.0).max(4.0).round() as u8,
                    );
                    if item_response.hovered() {
                        ui.painter()
                            .rect_filled(item_rect, item_cr, style.item_hover_bg);
                        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                    }

                    let text_x = item_rect.min.x + 6.0;
                    if is_selected {
                        let check_rect = egui::Rect::from_min_size(
                            egui::pos2(
                                item_rect.max.x - check_icon_size - 8.0,
                                item_rect.center().y - check_icon_size / 2.0,
                            ),
                            egui::vec2(check_icon_size, check_icon_size),
                        );
                        crate::icons::paint_icon::paint_icon(
                            ui.painter(),
                            check_rect,
                            &crate::icons::lucide_icon::LucideIcon::Check,
                            style.item_text,
                        );
                    }

                    ui.painter().galley(
                        egui::pos2(text_x, item_rect.center().y - galley.size().y / 2.0),
                        galley,
                        style.item_text,
                    );
                }

                if item_response.clicked() {
                    *self.selected = Some(option.clone());
                    egui::Popup::close_id(ui.ctx(), popup_id);
                    ui.ctx().request_repaint();
                }
            }
        });

        response
    }
}

// ---------------------------------------------------------------------------
// SelectValue — non-Option variant
// ---------------------------------------------------------------------------

impl<T: Clone + std::fmt::Display + PartialEq + 'static> egui::Widget
    for super::select::SelectValue<'_, T>
{
    fn ui(self, ui: &mut egui::Ui) -> egui::Response {
        let theme = crate::theme::shadcn_theme_ext::ShadcnThemeExt::shadcn_theme(ui.ctx());
        let style = super::select_style::resolve_select_style(&theme);

        let height: f32 = 32.0;
        let h_padding: f32 = 10.0;
        let chevron_width: f32 = 20.0;
        let width = self.width.unwrap_or(ui.available_width().min(200.0));
        let desired = egui::vec2(width, height);

        let (rect, response) = ui.allocate_exact_size(desired, egui::Sense::click());
        let popup_id = response.id.with("popup");

        if ui.is_rect_visible(rect) {
            let painter = ui.painter();
            let cr = egui::CornerRadius::same(style.corner_radius.round() as u8);
            let pressed = response.is_pointer_button_down_on();
            let trigger_bg = if pressed {
                crate::paint::interpolate_color::interpolate_color(
                    style.trigger_bg,
                    theme.accent,
                    0.85,
                )
            } else if response.hovered() {
                theme.accent
            } else {
                style.trigger_bg
            };
            let trigger_border = if response.hovered() || pressed {
                theme.ring
            } else {
                style.trigger_border
            };

            painter.rect_filled(rect, cr, trigger_bg);
            painter.rect_stroke(
                rect,
                cr,
                egui::Stroke::new(1.0, trigger_border),
                egui::epaint::StrokeKind::Inside,
            );

            let display_text = if let Some(ref override_text) = self.selected_text_override {
                override_text.clone()
            } else {
                self.selected.to_string()
            };

            let galley = painter.layout_no_wrap(
                display_text,
                egui::FontId::proportional(14.0),
                style.trigger_text,
            );
            let text_pos = egui::pos2(
                rect.min.x + h_padding,
                rect.center().y - galley.size().y / 2.0,
            );
            painter.galley(text_pos, galley, style.trigger_text);

            let icon_size: f32 = 14.0;
            let chevron_rect = egui::Rect::from_center_size(
                egui::pos2(rect.max.x - chevron_width / 2.0 - 2.0, rect.center().y),
                egui::vec2(icon_size, icon_size),
            );
            crate::icons::paint_icon::paint_icon(
                painter,
                chevron_rect,
                &crate::icons::lucide_icon::LucideIcon::ChevronDown,
                theme.muted_foreground,
            );

            if response.has_focus() {
                crate::paint::paint_focus_ring::paint_focus_ring(
                    painter,
                    rect,
                    style.corner_radius,
                    theme.ring,
                );
            }
        }

        if response.hovered() {
            ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
        }

        // The popup id derives from the trigger's auto-id. If the widget's
        // position in the UI tree ever changes (window/tab closed, layout
        // reordered), an old popup id can linger in `Memory::popups` for a
        // frame. `Memory::end_pass` prunes popups that were not re-asserted
        // with `keep_popup_open`, but if the popup was merely *rendered* in a
        // different place (e.g. the settings window reopened), the stale entry
        // survives. A stale entry would turn the first trigger click into a
        // "toggle-closed" no-op, so purge it explicitly: a popup that is open
        // in memory but whose area was not drawn last frame is stale.
        if egui::Popup::is_id_open(ui.ctx(), popup_id) && ui.ctx().read_response(popup_id).is_none()
        {
            egui::Popup::close_id(ui.ctx(), popup_id);
        }

        let toggle_cmd = if response.clicked() {
            // Always open on a trigger click instead of toggling: a stale (or
            // just-orphaned) entry in popup memory must never turn the first
            // click into a close. The popup still closes on outside clicks,
            // Esc, and item selection (see below).
            Some(egui::SetOpenCommand::Bool(true))
        } else {
            None
        };

        let popup_cr = style.corner_radius.round() as u8;
        let popup = egui::Popup::new(popup_id, ui.ctx().clone(), &response, ui.layer_id())
            .open_memory(toggle_cmd)
            .close_behavior(egui::PopupCloseBehavior::CloseOnClickOutside)
            .frame(
                egui::Frame::NONE
                    .fill(style.popover_bg)
                    .inner_margin(egui::Margin::same(4))
                    .corner_radius(egui::CornerRadius::same(popup_cr))
                    .stroke(egui::Stroke::new(1.0, style.popover_border))
                    .shadow(egui::Shadow {
                        offset: [0, 4],
                        blur: 12,
                        spread: 0,
                        color: egui::Color32::from_black_alpha(8),
                    }),
            );

        popup.show(|ui: &mut egui::Ui| {
            let popup_width = width.max(144.0);
            ui.set_min_width(popup_width);
            ui.set_max_width(popup_width);
            let check_icon_size: f32 = 12.0;

            for option in self.options {
                let is_selected = self.selected == option;
                let label = option.to_string();

                let galley = ui.painter().layout_no_wrap(
                    label.clone(),
                    egui::FontId::proportional(14.0),
                    style.item_text,
                );

                let item_height = galley.size().y + 8.0;
                let item_desired = egui::vec2(popup_width, item_height);
                let (item_rect, item_response) =
                    ui.allocate_exact_size(item_desired, egui::Sense::click());

                if ui.is_rect_visible(item_rect) {
                    let item_cr = egui::CornerRadius::same(
                        (style.corner_radius - 2.0).max(4.0).round() as u8,
                    );
                    if item_response.hovered() {
                        ui.painter()
                            .rect_filled(item_rect, item_cr, style.item_hover_bg);
                        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                    }

                    let text_x = item_rect.min.x + 6.0;
                    if is_selected {
                        let check_rect = egui::Rect::from_min_size(
                            egui::pos2(
                                item_rect.max.x - check_icon_size - 8.0,
                                item_rect.center().y - check_icon_size / 2.0,
                            ),
                            egui::vec2(check_icon_size, check_icon_size),
                        );
                        crate::icons::paint_icon::paint_icon(
                            ui.painter(),
                            check_rect,
                            &crate::icons::lucide_icon::LucideIcon::Check,
                            style.item_text,
                        );
                    }

                    ui.painter().galley(
                        egui::pos2(text_x, item_rect.center().y - galley.size().y / 2.0),
                        galley,
                        style.item_text,
                    );
                }

                if item_response.clicked() {
                    *self.selected = option.clone();
                    egui::Popup::close_id(ui.ctx(), popup_id);
                    ui.ctx().request_repaint();
                }
            }
        });

        response
    }
}

#[cfg(test)]
mod tests {
    use egui::{Context, Event, Modifiers, PointerButton, RawInput};

    use super::super::select::SelectValue;

    const OPTIONS: [&str; 3] = ["auto", "en", "fr"];

    fn draw(ctx: &Context, events: Vec<Event>, selected: &mut String) {
        let options: Vec<String> = OPTIONS.iter().map(|s| s.to_string()).collect();
        let _ = ctx.run_ui(
            RawInput {
                events,
                ..Default::default()
            },
            |ui| {
                SelectValue::new(selected, &options).width(200.0).show(ui);
            },
        );
    }

    fn press(ctx: &Context, pos: egui::Pos2, pressed: bool, selected: &mut String) {
        draw(
            ctx,
            vec![Event::PointerButton {
                pos,
                button: PointerButton::Primary,
                pressed,
                modifiers: Modifiers::NONE,
            }],
            selected,
        );
    }

    fn click(ctx: &Context, pos: egui::Pos2, selected: &mut String) {
        press(ctx, pos, true, selected);
        press(ctx, pos, false, selected);
    }

    /// The trigger rect is at the top-left of the content rect: (0,0)-(200,32).
    const TRIGGER: egui::Pos2 = egui::pos2(100.0, 20.0);
    /// Second item ("en") sits directly below the trigger inside the popup.
    const ITEM_EN: egui::Pos2 = egui::pos2(100.0, 76.0);
    /// Far away from both trigger and popup.
    const ELSEWHERE: egui::Pos2 = egui::pos2(600.0, 400.0);

    #[test]
    fn trigger_click_opens_popup() {
        let ctx = Context::default();
        let mut selected = "auto".to_string();
        draw(&ctx, vec![], &mut selected);
        click(&ctx, TRIGGER, &mut selected);
        assert!(
            egui::Popup::is_any_open(&ctx),
            "a trigger click must open the popup"
        );
    }

    #[test]
    fn item_click_selects_and_closes() {
        let ctx = Context::default();
        let mut selected = "auto".to_string();
        draw(&ctx, vec![], &mut selected);
        click(&ctx, TRIGGER, &mut selected);
        assert!(egui::Popup::is_any_open(&ctx));
        // Normal user pacing: let the popup render a few frames first.
        for _ in 0..4 {
            draw(&ctx, vec![], &mut selected);
        }
        click(&ctx, ITEM_EN, &mut selected);
        assert_eq!(selected, "en", "item click must select the option");
        assert!(
            !egui::Popup::is_any_open(&ctx),
            "item click must close the popup"
        );
    }

    /// egui sizes a freshly opened popup on an invisible "sizing pass" frame
    /// whose widgets are registered disabled; any press that lands on the very
    /// first visible frame therefore never reaches an item and would, with
    /// `CloseOnClick`, dismiss the popup without selecting anything. The popup
    /// must survive that click so the next click can select.
    #[test]
    fn fast_click_on_first_visible_frame_is_not_lost() {
        let ctx = Context::default();
        let mut selected = "auto".to_string();
        draw(&ctx, vec![], &mut selected);
        click(&ctx, TRIGGER, &mut selected);
        assert!(egui::Popup::is_any_open(&ctx));
        // Click an item immediately: no settling frames in between.
        click(&ctx, ITEM_EN, &mut selected);
        assert!(
            egui::Popup::is_any_open(&ctx),
            "a click eaten by the sizing-pass frame must not dismiss the popup"
        );
        // The follow-up click must work normally.
        for _ in 0..2 {
            draw(&ctx, vec![], &mut selected);
        }
        click(&ctx, ITEM_EN, &mut selected);
        assert_eq!(selected, "en");
        assert!(!egui::Popup::is_any_open(&ctx));
    }

    #[test]
    fn outside_click_closes_popup() {
        let ctx = Context::default();
        let mut selected = "auto".to_string();
        draw(&ctx, vec![], &mut selected);
        click(&ctx, TRIGGER, &mut selected);
        assert!(egui::Popup::is_any_open(&ctx));
        for _ in 0..4 {
            draw(&ctx, vec![], &mut selected);
        }
        click(&ctx, ELSEWHERE, &mut selected);
        assert!(
            !egui::Popup::is_any_open(&ctx),
            "an outside click must close the popup"
        );
    }

    #[test]
    fn trigger_click_while_open_closes() {
        let ctx = Context::default();
        let mut selected = "auto".to_string();
        draw(&ctx, vec![], &mut selected);
        click(&ctx, TRIGGER, &mut selected);
        assert!(egui::Popup::is_any_open(&ctx));
        for _ in 0..4 {
            draw(&ctx, vec![], &mut selected);
        }
        // The trigger sits outside the popup, so re-clicking it dismisses it.
        click(&ctx, TRIGGER, &mut selected);
        assert!(!egui::Popup::is_any_open(&ctx));
    }
}

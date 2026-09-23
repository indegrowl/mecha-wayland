use app::{App, Context, Spawner, Widget};
use geometry::Color;

use crate::typography::{TextVariant, TypographyStyle};
use crate::{ColorRole, MechanixTheme};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct ThemeChanged;
impl app::Signal for ThemeChanged {}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct ApplyTheme;
impl app::Event for ApplyTheme {}

pub fn on_theme_changed(app: &mut App, _: &ThemeChanged) {
    let mut targets = vec![app.root()];
    targets.extend(app.tree().descendants(app.root()));
    app.emit(ApplyTheme, targets);
}

pub trait ThemeReader {
    fn theme(&self) -> &MechanixTheme;

    #[inline]
    fn color(&self, role: ColorRole) -> Color {
        self.theme().color(role)
    }

    #[inline]
    fn typography(&self, variant: TextVariant) -> TypographyStyle {
        self.theme().typography(variant)
    }
}

pub trait ContextThemeExt: ThemeReader {
    fn set_theme(&mut self, theme: MechanixTheme);
    fn update_theme<F: FnOnce(&mut MechanixTheme)>(&mut self, f: F);
}

impl<W: Widget> ThemeReader for Context<'_, W> {
    #[inline]
    fn theme(&self) -> &MechanixTheme {
        self.resource::<MechanixTheme>()
    }
}

impl<W: Widget> ContextThemeExt for Context<'_, W> {
    fn set_theme(&mut self, theme: MechanixTheme) {
        *self.resource_mut::<MechanixTheme>() = theme;
        self.signal(ThemeChanged);
    }

    fn update_theme<F: FnOnce(&mut MechanixTheme)>(&mut self, f: F) {
        f(&mut self.resource_mut::<MechanixTheme>());
        self.signal(ThemeChanged);
    }
}

pub trait AppThemeExt: ThemeReader {
    fn set_theme(&mut self, theme: MechanixTheme);
    fn update_theme<F: FnOnce(&mut MechanixTheme)>(&mut self, f: F);
}

impl ThemeReader for App {
    #[inline]
    fn theme(&self) -> &MechanixTheme {
        self.resource::<MechanixTheme>()
    }
}

impl AppThemeExt for App {
    fn set_theme(&mut self, theme: MechanixTheme) {
        *self.resource_mut::<MechanixTheme>() = theme;
        self.signal(ThemeChanged);
    }

    fn update_theme<F: FnOnce(&mut MechanixTheme)>(&mut self, f: F) {
        f(&mut self.resource_mut::<MechanixTheme>());
        self.signal(ThemeChanged);
    }
}

pub trait SpawnerThemeExt<W: Widget>: ThemeReader {
    fn on_theme(
        &mut self,
        target: impl Into<app::NodeId>,
        handler: impl FnMut(&mut Context<'_, W>) + 'static,
    ) -> &mut Self;
}

impl<W: Widget> ThemeReader for Spawner<'_, W> {
    #[inline]
    fn theme(&self) -> &MechanixTheme {
        self.resource::<MechanixTheme>()
    }
}

impl<W: Widget> SpawnerThemeExt<W> for Spawner<'_, W> {
    fn on_theme(
        &mut self,
        target: impl Into<app::NodeId>,
        mut handler: impl FnMut(&mut Context<'_, W>) + 'static,
    ) -> &mut Self {
        self.on::<ApplyTheme>(target, move |ctx, _| handler(ctx))
    }
}

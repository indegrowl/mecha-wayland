use app::prelude::*;
use theme::prelude::*;

struct TestWidget {
    theme_updates: usize,
}

struct TestWidgetBuilder;
impl Build for TestWidgetBuilder {
    type Widget = TestWidget;
}

impl Widget for TestWidget {
    type Builder = TestWidgetBuilder;

    fn build(_: TestWidgetBuilder, me: Handle<Self>, s: &mut Spawner<'_, Self>) -> Self {
        s.on_theme(me, |ctx| {
            ctx.me().theme_updates += 1;
        });

        TestWidget { theme_updates: 0 }
    }
}

#[test]
fn theme_installation_and_signals() {
    let mut app = App::new();
    app.add_module(MechanixTheme::dark());

    assert_eq!(app.theme().mode(), ThemeMode::Dark);
    assert_ne!(app.color(ColorRole::Primary), app.color(ColorRole::Surface));

    let handle = app.spawn(app.root(), TestWidgetBuilder);
    app.tick();
    assert_eq!(app.widget::<TestWidget>(handle).unwrap().theme_updates, 0);

    app.set_theme(MechanixTheme::light());
    assert_eq!(app.theme().mode(), ThemeMode::Light);

    app.tick();
    assert_eq!(app.widget::<TestWidget>(handle).unwrap().theme_updates, 1);

    app.update_theme(|t| {
        t.colors.primary = geometry::Color::BLACK;
    });

    app.tick();
    assert_eq!(app.widget::<TestWidget>(handle).unwrap().theme_updates, 2);
    assert_eq!(app.color(ColorRole::Primary), geometry::Color::BLACK);
}

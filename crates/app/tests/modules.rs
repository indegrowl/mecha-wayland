//! Modules: `install` against the app in call order, chaining, the runner
//! a module sets, and `OnChanged<C>` end to end.

use std::cell::RefCell;

use app::prelude::*;

// ── fixtures ────────────────────────────────────────────────────────────

thread_local! {
    static LOG: RefCell<Vec<String>> = const { RefCell::new(Vec::new()) };
}

fn log(s: impl Into<String>) {
    LOG.with(|l| l.borrow_mut().push(s.into()));
}

fn take_log() -> Vec<String> {
    LOG.with(|l| std::mem::take(&mut *l.borrow_mut()))
}

#[derive(Default, Debug, Clone, Copy, PartialEq)]
struct Layout {
    x: f32,
    y: f32,
}
impl Component for Layout {}

struct Sync;
impl Signal for Sync {}

/// A component, and a system that writes it from the module's fields.
struct LayoutModule {
    width: u32,
    height: u32,
}
impl Module for LayoutModule {
    fn install(self, app: &mut App) {
        fn on_sync(app: &mut App, _: &Sync) {
            let root = app.root();
            let mut layout = app.component_mut::<Layout>(root).unwrap();
            layout.x *= 2.0;
            layout.y *= 2.0;
        }
        log(format!("layout {}x{}", self.width, self.height));
        let root = app.root();
        app.register_component::<Layout>().system(on_sync);
        let mut layout = app.component_mut::<Layout>(root).unwrap();
        *layout = Layout {
            x: self.width as f32,
            y: self.height as f32,
        };
    }
}

/// Depends on `LayoutModule`: reads its component at install time.
struct HalfModule;
impl Module for HalfModule {
    fn install(self, app: &mut App) {
        log("half");
        let root = app.root();
        let mut layout = app.component_mut::<Layout>(root).unwrap();
        layout.x /= 2.0;
        layout.y /= 2.0;
    }
}

/// Owns the loop: one tick, then report and return.
struct OneTickRunner;
impl Module for OneTickRunner {
    fn install(self, app: &mut App) {
        fn one_tick(mut app: App) {
            app.tick();
            log("runner done");
        }
        fn on_tick(_: &mut App, _: &Tick) {
            log("tick");
        }
        log("runner");
        app.system(on_tick).set_runner(one_tick);
    }
}

fn root_layout(app: &App) -> Layout {
    *app.component::<Layout>(app.root()).unwrap()
}

// ── install ─────────────────────────────────────────────────────────────

#[test]
fn install_registers_a_component_and_a_system_from_the_modules_fields() {
    let mut app = App::new();
    app.add_module(LayoutModule {
        width: 800,
        height: 600,
    });
    assert_eq!(root_layout(&app), Layout { x: 800.0, y: 600.0 });
    app.signal(Sync);
    app.flush();
    assert_eq!(
        root_layout(&app),
        Layout {
            x: 1600.0,
            y: 1200.0
        },
        "system ran"
    );
}

#[test]
fn add_module_chains_installs_in_call_order_and_a_later_module_builds_on_an_earlier_one() {
    let mut app = App::new();
    app.add_module(LayoutModule {
        width: 800,
        height: 600,
    })
    .add_module(HalfModule)
    .add_module(OneTickRunner);
    assert_eq!(take_log(), ["layout 800x600", "half", "runner"]);
    assert_eq!(root_layout(&app), Layout { x: 400.0, y: 300.0 });
}

#[test]
#[should_panic(expected = "not registered")]
fn a_module_added_before_its_dependency_fails_at_install() {
    let mut app = App::new();
    app.add_module(HalfModule);
}

#[test]
#[should_panic(expected = "already registered")]
fn adding_a_module_twice_is_not_deduped() {
    let mut app = App::new();
    app.add_module(LayoutModule {
        width: 1,
        height: 1,
    })
    .add_module(LayoutModule {
        width: 1,
        height: 1,
    });
}

// ── runner ──────────────────────────────────────────────────────────────

#[test]
fn a_module_sets_the_runner_and_run_returns_when_it_does() {
    let mut app = App::new();
    app.add_module(OneTickRunner);
    take_log();
    app.run();
    assert_eq!(take_log(), ["tick", "runner done"]);
}

#[test]
#[should_panic(expected = "runner already set")]
fn two_modules_setting_the_runner_is_a_configuration_error() {
    struct LoopOwner;
    impl Module for LoopOwner {
        fn install(self, app: &mut App) {
            app.set_runner(|_| {});
        }
    }
    let mut app = App::new();
    app.add_module(LoopOwner).add_module(LoopOwner);
}

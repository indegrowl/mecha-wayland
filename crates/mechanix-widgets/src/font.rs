use app::{App, Context, Spawner, Widget};
use atlas::FontId;
use theme::FontWeight;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FontBook {
    entries: Vec<(u16, FontId)>,
}

impl FontBook {
    pub fn new(regular: FontId) -> Self {
        Self {
            entries: vec![(FontWeight::Regular.value(), regular)],
        }
    }

    pub fn light(self, font: FontId) -> Self {
        self.at(FontWeight::Light.value(), font)
    }

    pub fn regular(self, font: FontId) -> Self {
        self.at(FontWeight::Regular.value(), font)
    }

    pub fn medium(self, font: FontId) -> Self {
        self.at(FontWeight::Medium.value(), font)
    }

    pub fn semi_bold(self, font: FontId) -> Self {
        self.at(FontWeight::SemiBold.value(), font)
    }

    pub fn bold(self, font: FontId) -> Self {
        self.at(FontWeight::Bold.value(), font)
    }

    pub fn extra_bold(self, font: FontId) -> Self {
        self.at(FontWeight::ExtraBold.value(), font)
    }

    /// Register a font face for any numeric weight (100–900) or [`FontWeight`].
    pub fn at(mut self, weight: impl IntoWeight, font: FontId) -> Self {
        let w = weight.into_weight();
        match self.entries.binary_search_by_key(&w, |&(wt, _)| wt) {
            Ok(i) => self.entries[i].1 = font,
            Err(i) => self.entries.insert(i, (w, font)),
        }
        self
    }

    /// Resolves the closest matching [`FontId`] for `weight` using CSS nearest-neighbor rules.
    pub fn resolve(&self, weight: impl IntoWeight) -> FontId {
        let w = weight.into_weight();
        debug_assert!(!self.entries.is_empty(), "FontBook is always non-empty");

        match self.entries.binary_search_by_key(&w, |&(wt, _)| wt) {
            Ok(i) => self.entries[i].1,
            Err(0) => {
                #[cfg(debug_assertions)]
                eprintln!(
                    "FontBook: weight {w} is lighter than all registered faces; falling back to {}",
                    self.entries[0].0
                );
                self.entries[0].1
            }
            Err(i) if i == self.entries.len() => {
                let last = self.entries.last().unwrap();
                #[cfg(debug_assertions)]
                eprintln!(
                    "FontBook: weight {w} is heavier than all registered faces; falling back to {}",
                    last.0
                );
                last.1
            }
            Err(i) => {
                let (_, lo_id) = self.entries[i - 1];
                let (_, hi_id) = self.entries[i];
                if w >= 400 { hi_id } else { lo_id }
            }
        }
    }
}

impl app::Resource for FontBook {}

pub trait IntoWeight: private::Sealed {
    fn into_weight(self) -> u16;
}

mod private {
    pub trait Sealed {}
    impl Sealed for u16 {}
    impl Sealed for super::FontWeight {}
}

impl IntoWeight for u16 {
    #[inline]
    fn into_weight(self) -> u16 {
        self
    }
}

impl IntoWeight for FontWeight {
    #[inline]
    fn into_weight(self) -> u16 {
        self.value()
    }
}

pub trait FontContextExt {
    fn font(&self, weight: impl IntoWeight) -> FontId;
}

impl<W: Widget> FontContextExt for Spawner<'_, W> {
    #[inline]
    fn font(&self, weight: impl IntoWeight) -> FontId {
        self.resource::<FontBook>().resolve(weight)
    }
}

impl<W: Widget> FontContextExt for Context<'_, W> {
    #[inline]
    fn font(&self, weight: impl IntoWeight) -> FontId {
        self.resource::<FontBook>().resolve(weight)
    }
}

impl FontContextExt for App {
    #[inline]
    fn font(&self, weight: impl IntoWeight) -> FontId {
        self.resource::<FontBook>().resolve(weight)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fid(n: u32) -> FontId {
        FontId(n as u16)
    }

    fn book() -> FontBook {
        FontBook::new(fid(400))
            .light(fid(300))
            .medium(fid(500))
            .semi_bold(fid(600))
            .bold(fid(700))
    }

    #[test]
    fn exact_match() {
        let b = book();
        assert_eq!(b.resolve(300u16), fid(300));
        assert_eq!(b.resolve(400u16), fid(400));
        assert_eq!(b.resolve(700u16), fid(700));
        assert_eq!(b.resolve(FontWeight::Light), fid(300));
        assert_eq!(b.resolve(FontWeight::Regular), fid(400));
        assert_eq!(b.resolve(FontWeight::Bold), fid(700));
    }

    #[test]
    fn nearest_neighbour_above_400() {
        let b = book();
        assert_eq!(b.resolve(550u16), fid(600));
        assert_eq!(b.resolve(450u16), fid(500));
    }

    #[test]
    fn nearest_neighbour_below_400() {
        let b = book();
        assert_eq!(b.resolve(350u16), fid(300));
    }

    #[test]
    fn clamp_below_all() {
        let b = book();
        assert_eq!(b.resolve(100u16), fid(300));
        assert_eq!(b.resolve(FontWeight::Thin), fid(300));
    }

    #[test]
    fn clamp_above_all() {
        let b = book();
        assert_eq!(b.resolve(800u16), fid(700));
        assert_eq!(b.resolve(FontWeight::ExtraBold), fid(700));
    }

    #[test]
    fn custom_numeric_weight() {
        let b = FontBook::new(fid(400)).at(350u16, fid(350));
        assert_eq!(b.resolve(350u16), fid(350));
        assert_eq!(b.resolve(340u16), fid(350));
    }

    #[test]
    fn replace_existing_weight() {
        let b = FontBook::new(fid(400)).light(fid(300)).at(300u16, fid(999));
        assert_eq!(b.resolve(FontWeight::Light), fid(999));
    }
}

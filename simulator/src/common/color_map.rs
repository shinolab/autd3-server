use scarlet::color::RGBColor;
use scarlet::colormap::ColorMap as _;
use serde::{Deserialize, Serialize};
use strum::EnumIter;

#[derive(Serialize, Deserialize, Debug, Clone, Copy, Hash, PartialEq, Eq, EnumIter)]
pub enum ColorMap {
    Bluered,
    Breeze,
    Circle,
    Earth,
    Hell,
    Inferno,
    Magma,
    Mist,
    Plasma,
    Turbo,
    Viridis,
}

impl ColorMap {
    pub fn color_map(&self, iter: impl IntoIterator<Item = f64>) -> Vec<RGBColor> {
        match self {
            Self::Viridis => scarlet::colormap::ListedColorMap::viridis().transform(iter),
            Self::Magma => scarlet::colormap::ListedColorMap::magma().transform(iter),
            Self::Inferno => scarlet::colormap::ListedColorMap::inferno().transform(iter),
            Self::Plasma => scarlet::colormap::ListedColorMap::plasma().transform(iter),
            Self::Bluered => scarlet::colormap::ListedColorMap::bluered().transform(iter),
            Self::Breeze => scarlet::colormap::ListedColorMap::breeze().transform(iter),
            Self::Circle => scarlet::colormap::ListedColorMap::circle().transform(iter),
            Self::Earth => scarlet::colormap::ListedColorMap::earth().transform(iter),
            Self::Hell => scarlet::colormap::ListedColorMap::hell().transform(iter),
            Self::Mist => scarlet::colormap::ListedColorMap::mist().transform(iter),
            Self::Turbo => scarlet::colormap::ListedColorMap::turbo().transform(iter),
        }
    }
}

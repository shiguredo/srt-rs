/// sansio パターンで時間を外部から与えるためのタイムスタンプ型
///
/// マイクロ秒単位の時刻を表す。SRT プロトコルでは接続確立からの
/// 相対時刻をマイクロ秒単位で扱う。
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub struct Timestamp(pub u64);

impl Timestamp {
    /// マイクロ秒からタイムスタンプを生成する
    pub fn from_micros(micros: u64) -> Self {
        Self(micros)
    }

    /// タイムスタンプをマイクロ秒として取得する
    pub fn as_micros(&self) -> u64 {
        self.0
    }

    /// タイムスタンプをミリ秒として取得する
    pub fn as_millis(&self) -> u64 {
        self.0 / 1000
    }

    /// 2 つのタイムスタンプの差分をマイクロ秒で取得する
    pub fn saturating_sub(&self, other: Self) -> u64 {
        self.0.saturating_sub(other.0)
    }

    /// タイムスタンプにマイクロ秒を加算する
    pub fn add_micros(&self, micros: u64) -> Self {
        Self(self.0.saturating_add(micros))
    }

    /// タイムスタンプにミリ秒を加算する
    pub fn add_millis(&self, millis: u64) -> Self {
        self.add_micros(millis * 1000)
    }
}

impl std::ops::Add<u64> for Timestamp {
    type Output = Self;

    fn add(self, rhs: u64) -> Self::Output {
        Self(self.0.saturating_add(rhs))
    }
}

impl std::ops::Sub for Timestamp {
    type Output = u64;

    fn sub(self, rhs: Self) -> Self::Output {
        self.0.saturating_sub(rhs.0)
    }
}

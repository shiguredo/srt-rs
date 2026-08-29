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
    ///
    /// `millis` をマイクロ秒に換算した値と元のマイクロ秒値の合計が `u64` の範囲に
    /// 収まらない場合は panic せず、結果は `u64::MAX` マイクロ秒で飽和する。
    pub fn add_millis(&self, millis: u64) -> Self {
        // エラーを返さず飽和させるのは、add_micros も saturating_add を使うこの型の一貫した
        // 挙動に合わせるため。
        self.add_micros(millis.saturating_mul(1000))
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

#[cfg(test)]
mod tests {
    use super::*;

    // ミリ秒からマイクロ秒への変換がオーバーフローし始める境界値。
    // u64::MAX を 1000 で割った商を超えた瞬間に millis * 1000 が u64 の範囲に収まらなくなる。
    const OVERFLOW_BOUNDARY_MILLIS: u64 = u64::MAX / 1000 + 1;

    #[test]
    fn test_add_millis_saturates_over_boundary() {
        // 旧実装 (millis * 1000) では、この境界値で debug ビルドは panic し release ビルドは
        // 値が wrap した。saturating_mul に変えた現在は元のマイクロ秒値によらず結果が飽和する。
        let ts = Timestamp::from_micros(7).add_millis(OVERFLOW_BOUNDARY_MILLIS);
        assert_eq!(ts.as_micros(), u64::MAX);

        // millis 自体が u64::MAX の極値でも同じ。
        let ts = Timestamp::from_micros(0).add_millis(u64::MAX);
        assert_eq!(ts.as_micros(), u64::MAX);
    }

    #[test]
    fn test_add_millis_saturates_on_sum_overflow() {
        // 換算後のマイクロ秒数が範囲内でも、元のマイクロ秒値との合計が収まらなければ飽和する。
        let ts = Timestamp::from_micros(u64::MAX - 500_000).add_millis(1_000);
        assert_eq!(ts.as_micros(), u64::MAX);
    }

    #[test]
    fn test_add_millis_below_boundary_is_exact() {
        // 境界値より 1 小さい値では飽和せず正確に変換される。期待値は (u64::MAX / 1000) * 1000。
        // 常に u64::MAX を返すような、飽和が早すぎる実装を検出するのが狙い。
        let ts = Timestamp::from_micros(0).add_millis(OVERFLOW_BOUNDARY_MILLIS - 1);
        assert_eq!(ts.as_micros(), 18_446_744_073_709_551_000);

        // 飽和しない領域で元のマイクロ秒値が 0 以外となるケースを確認する断言はここだけ。
        let ts = Timestamp::from_micros(500).add_millis(1_500);
        assert_eq!(ts.as_micros(), 1_500_500);
    }
}

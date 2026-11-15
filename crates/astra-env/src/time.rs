//! Tempo: data e hora UTC, data juliana e hora sideral média de Greenwich.

use core::f64::consts::PI;

/// Data juliana de uma data e hora UTC do calendário gregoriano.
/// Fonte: Vallado, "Fundamentals of Astrodynamics and Applications", 4ª ed., alg. 14.
#[must_use]
pub fn julian_date(year: i32, month: u32, day: u32, hour: u32, min: u32, sec: f64) -> f64 {
    let (y, m) = (f64::from(year), f64::from(month));
    367.0 * y - (7.0 * (y + ((m + 9.0) / 12.0).floor()) / 4.0).floor()
        + (275.0 * m / 9.0).floor()
        + f64::from(day)
        + 1_721_013.5
        + ((sec / 60.0 + f64::from(min)) / 60.0 + f64::from(hour)) / 24.0
}

/// Hora sideral média de Greenwich (rad) para a data juliana `jd` (UT1 ≈ UTC):
/// θ = 67310,54841 s + (876600·3600 + 8640184,812866)·T + 0,093104·T² − 6,2e-6·T³,
/// T em séculos julianos desde J2000.
/// Fonte: Vallado, 4ª ed., eq. 3-47 (modelo IAU-82).
#[must_use]
pub fn gmst(jd: f64) -> f64 {
    let t = (jd - 2_451_545.0) / 36_525.0;
    let s = 67_310.548_41 + (876_600.0 * 3600.0 + 8_640_184.812_866) * t + 0.093_104 * t * t
        - 6.2e-6 * t * t * t;
    (s.rem_euclid(86_400.0) / 240.0)
        .to_radians()
        .rem_euclid(2.0 * PI)
}

/// Lê "AAAA-MM-DDThh:mm[:ss[.s]]Z" (UTC). Devolve a data juliana.
///
/// # Errors
/// Mensagem quando o texto não segue o formato.
pub fn parse_utc(s: &str) -> Result<f64, String> {
    let bad = || format!("data e hora inválidas: \"{s}\" (use AAAA-MM-DDThh:mm:ssZ)");
    let s2 = s.trim().trim_end_matches('Z');
    let (date, time) = s2.split_once('T').ok_or_else(bad)?;
    let d: Vec<&str> = date.split('-').collect();
    let t: Vec<&str> = time.split(':').collect();
    if d.len() != 3 || t.len() < 2 {
        return Err(bad());
    }
    let n = |x: &str| x.parse::<u32>().map_err(|_| bad());
    let (y, mo, da) = (d[0].parse::<i32>().map_err(|_| bad())?, n(d[1])?, n(d[2])?);
    let (h, mi) = (n(t[0])?, n(t[1])?);
    let sec = t
        .get(2)
        .map_or(Ok(0.0), |x| x.parse::<f64>().map_err(|_| bad()))?;
    if !(1..=12).contains(&mo)
        || !(1..=31).contains(&da)
        || h > 23
        || mi > 59
        || !(0.0..61.0).contains(&sec)
    {
        return Err(bad());
    }
    Ok(julian_date(y, mo, da, h, mi, sec))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Vallado, exemplo 3-5: 1992-08-20 12:14:00 UT1 → JD 2448855,009722 e
    /// GMST = 152,578788°.
    #[test]
    fn vallado_gmst_example() {
        let jd = julian_date(1992, 8, 20, 12, 14, 0.0);
        assert!((jd - 2_448_855.009_722).abs() < 1e-6);
        assert!((gmst(jd).to_degrees() - 152.578_788).abs() < 1e-5);
        assert_eq!(parse_utc("1992-08-20T12:14:00Z").unwrap(), jd);
        assert!(parse_utc("1992-13-20T12:14Z").is_err());
        assert!(parse_utc("ontem").is_err());
        // J2000: 2000-01-01 12:00 TT ≈ JD 2451545.
        assert!((julian_date(2000, 1, 1, 12, 0, 0.0) - 2_451_545.0).abs() < 1e-9);
    }
}

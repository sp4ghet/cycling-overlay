/// Earth mean radius used for haversine distance (WGS84 mean).
const EARTH_RADIUS_M: f64 = 6_371_000.0;

/// Great-circle distance in meters between two WGS84 points given in decimal degrees.
pub(crate) fn haversine_m(lat1_deg: f64, lon1_deg: f64, lat2_deg: f64, lon2_deg: f64) -> f64 {
    let phi1 = lat1_deg.to_radians();
    let phi2 = lat2_deg.to_radians();
    let dphi = (lat2_deg - lat1_deg).to_radians();
    let dlam = (lon2_deg - lon1_deg).to_radians();
    let a = (dphi / 2.0).sin().powi(2)
        + phi1.cos() * phi2.cos() * (dlam / 2.0).sin().powi(2);
    let c = 2.0 * a.sqrt().asin();
    EARTH_RADIUS_M * c
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn haversine_km_matches_known() {
        // London → Paris is ~344 km (real-world geodesic distance).
        let d = haversine_m(51.5074, -0.1278, 48.8566, 2.3522);
        assert!((d - 343_550.0).abs() < 1_000.0, "got {} m", d);
    }

    #[test]
    fn haversine_zero_for_same_point() {
        let d = haversine_m(35.77, 139.72, 35.77, 139.72);
        assert!(d.abs() < 0.01, "got {} m", d);
    }
}

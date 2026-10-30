//! Where this machine is, roughly, from its public IP address, as Breezy
//! Weather does: IP.SB's `geoip`, which needs no key.
//! Privacy policy: https://ip.sb/privacy-policy/

use serde::Deserialize;

use super::Location;

pub const URL: &str = "https://api.ip.sb/geoip";

/// A location found from the IP address.
#[derive(Debug, Clone, PartialEq)]
pub struct Place {
    pub location: Location,
    pub city: Option<String>,
}

#[derive(Deserialize)]
struct Response {
    latitude: Option<f64>,
    longitude: Option<f64>,
    city: Option<String>,
}

/// The place in a response from `URL`.
pub fn parse(body: &str) -> Result<Place, String> {
    let response: Response = serde_json::from_str(body).map_err(|e| e.to_string())?;
    let (Some(latitude), Some(longitude)) = (response.latitude, response.longitude) else {
        return Err("no coordinates".to_owned());
    };
    // What IP.SB gives when it doesn't know; Breezy reads it the same way.
    if latitude == 0.0 && longitude == 0.0 {
        return Err("no location for this IP address".to_owned());
    }
    Ok(Place {
        location: Location {
            latitude,
            longitude,
        },
        city: response.city.filter(|city| !city.is_empty()),
    })
}

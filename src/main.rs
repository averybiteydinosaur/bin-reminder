use chrono::{Duration, Local, NaiveDate};
use reqwest::blocking::{multipart, Client};
use std::error::Error;

fn initiate_client() -> Result<Client, reqwest::Error> {
    Client::builder()
        .user_agent("Mozilla/5.0 (X11; Linux x86_64; rv:128.0) Gecko/20100101 Firefox/128.0")
        .build()
}

fn query_bin_data(client: Client) -> Result<String, Box<dyn Error>> {
    let url = env!("LOOKUP_URL");
    Ok(client.get(url).send()?.text()?)
}

fn get_coded_pair(chunk: &[char]) -> Result<(String, char), Box<dyn Error>> {
    let coded_date = chunk
        .get(0..4)
        .ok_or("Coded String length not a multiple of 5")?
        .iter()
        .collect::<String>();
    let bin_code = chunk
        .get(4)
        .ok_or("Coded String length not a multiple of 5")?
        .to_owned();
    Ok((coded_date, bin_code))
}

fn get_coded_pairs(coded_data: String) -> Result<Vec<(String, char)>, Box<dyn Error>> {
    //Split each line into 5 digit segments, first four digits are for the encoded date, fifth digit is the letter for bin type
    coded_data
        .split(",")
        .collect::<Vec<&str>>()
        .get(1)
        .ok_or("Failed to split on ','")?
        .chars()
        .collect::<Vec<char>>()
        .chunks(5)
        .map(get_coded_pair) //TODO stop panic here
        .collect()
}

fn get_schedule(schedule_string: String) -> Result<Vec<(NaiveDate, String)>, Box<dyn Error>> {
    get_coded_schedule(schedule_string)?
        .into_iter()
        .map(decode_data)
        .collect()
}

fn get_coded_schedule(text_result: String) -> Result<Vec<(String, char)>, Box<dyn Error>> {
    let address_code = env!("ADDRESS_CODE");

    for line in text_result.lines() {
        if line.starts_with(address_code) {
            return get_coded_pairs(line.to_owned());
        }
    }
    Err("No result found for specified property")?
}

fn format_bin(bin_code: char) -> String {
    return match bin_code {
        'B' => "Black Bin".to_owned(),
        'G' => "Green Bin".to_owned(),
        'R' => "Brown Bin".to_owned(),
        other => format!("Unknown Bin '{}'", other).to_owned(),
    };
}

fn decode_date(coded_date: String) -> Result<NaiveDate, Box<dyn Error>> {
    let date = i32::from_str_radix(&coded_date, 36)?.to_string(); //data originally encoded in base 36

    //convert from string in format yymmdd
    let formatted_date = NaiveDate::from_ymd_opt(
        format!("20{}", &date[0..2]).parse()?,
        date[2..4].parse()?,
        date[4..6].parse()?,
    )
    .ok_or("Date conversion failure")?;
    Ok(formatted_date)
}

fn decode_data(coded_data: (String, char)) -> Result<(NaiveDate, String), Box<dyn Error>> {
    let (coded_date, bin_code) = coded_data;

    let decoded_date = decode_date(coded_date)?;
    let bin = format_bin(bin_code);

    Ok((decoded_date, bin))
}

fn get_tomorrows_bin(schedule: Vec<(NaiveDate, String)>) -> Option<String> {
    let date_tomorrow = Local::now().date_naive() + Duration::days(1);

    for (date, bin) in schedule {
        if date == date_tomorrow {
            return Some(bin);
        }
    }
    None
}

fn get_bin(client: Client) -> Result<Option<String>, Box<dyn Error>> {
    let site_response = query_bin_data(client)?;
    let schedule = get_schedule(site_response)?;
    Ok(get_tomorrows_bin(schedule))
}

fn send_notification(client: Client, message: String) {
    let url = env!("NOTIFICATION_URL");

    let form = multipart::Form::new()
        .text("title", "Bin Reminder")
        .text("message", message)
        .text("priority", "5");

    //errors sent via notification, if ok then no issue, if error would need to send via notification anyway.
    _ = client.post(url).multipart(form).send();
}

fn main() {
    let client = initiate_client().expect("Failed to create client"); //Panic on failure, as no client to send error message on
    match get_bin(client.clone()) {
        Err(e) => send_notification(client.clone(), format!("Error: {}", e)),
        Ok(result) => match result {
            Some(bin) => send_notification(client.clone(), format!("Put out {} for tomorrow", bin)),
            None => (),
        },
    }
}

#[cfg(test)]
mod tests {

    use super::*;

    #[test]
    fn test_format_bin() {
        assert_eq!(format_bin('B'), "Black Bin".to_owned());
        assert_eq!(format_bin('G'), "Green Bin".to_owned());
        assert_eq!(format_bin('R'), "Brown Bin".to_owned());
        assert_eq!(format_bin('T'), "Unknown Bin 'T'".to_owned());
    }

    #[test]
    fn test_decode_date() {
        let mut response = decode_date("559H".to_owned());
        let expected = NaiveDate::from_ymd_opt(2024, 01, 01).unwrap();
        assert_eq!(response.unwrap(), expected);

        response = decode_date("559I".to_owned());
        assert_ne!(response.unwrap(), expected);

        assert!(decode_date("559G".to_owned()).is_err())
    }

    #[test]
    fn test_get_coded_pair() {
        let mut response = get_coded_pair(&['a', 'b', 'c', 'd', 'e']);
        let expected = ("abcd".to_owned(), 'e');
        assert_eq!(response.unwrap(), expected);

        response = get_coded_pair(&['a', 'b', 'c', 'd']);
        assert!(response.is_err());
    }

    #[test]
    fn test_get_coded_pairs() {
        let mut response = get_coded_pairs("test,abcdefghij".to_owned());
        let expected = vec![("abcd".to_owned(), 'e'), ("fghi".to_owned(), 'j')];
        assert_eq!(response.unwrap(), expected);

        response = get_coded_pairs("test,abcdefghi".to_owned());
        assert!(response.is_err());
    }
}

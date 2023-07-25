use rand::Rng;
#[cfg(target_os = "linux")]
use std::os::unix::fs::PermissionsExt;
use std::process::Command;
use thirtyfour::{DesiredCapabilities, WebDriver};

/// Fetches a new ChromeDriver executable and patches it to prevent detection.
/// Returns a WebDriver instance.
pub async fn chrome() -> Result<WebDriver, Box<dyn std::error::Error>> {
    let postfix = if cfg!(windows) { ".exe" } else { "" };
    let chromedriver_exe_name = format!("chromedriver{postfix}");
    let chromedriver_patched_name = format!("chromedriver_PATCHED{postfix}");

    if std::path::Path::new(&chromedriver_patched_name).exists() {
        println!("Detected patched chromedriver executable!");
    } else {
        if std::path::Path::new(&chromedriver_exe_name).exists() {
            println!("ChromeDriver already exists!");
        } else {
            println!("ChromeDriver does not exist! Fetching...");
            let client = reqwest::Client::new();
            fetch_chromedriver(&client).await.unwrap();
        }

        println!("Starting ChromeDriver executable patch...");
        let f = std::fs::read(&chromedriver_exe_name).unwrap();
        let mut new_chromedriver_bytes = f.clone();
        let mut total_cdc = String::from("");
        let mut cdc_pos_list = Vec::new();
        let mut is_cdc_present = false;
        let mut patch_ct = 0;

        for i in 0..f.len() - 3 {
            if "cdc_"
                == format!(
                    "{}{}{}{}",
                    f[i] as char,
                    f[i + 1] as char,
                    f[i + 2] as char,
                    f[i + 3] as char
                )
                .as_str()
            {
                for x in i..i + 22 {
                    total_cdc.push_str(&(f[x] as char).to_string());
                }
                is_cdc_present = true;
                cdc_pos_list.push(i);
                total_cdc = String::from("");
            }
        }
        if is_cdc_present {
            println!("Found cdcs!")
        } else {
            println!("No cdcs were found!")
        }
        let get_random_char = || -> char {
            "abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ"
                .chars()
                .collect::<Vec<char>>()[rand::thread_rng().gen_range(0..48)]
        };
        for i in cdc_pos_list {
            for x in i..i + 22 {
                new_chromedriver_bytes[x] = get_random_char() as u8;
            }
            patch_ct += 1;
        }
        println!("Patched {} cdcs!", patch_ct);

        println!("Starting to write to binary file...");
        let _file = std::fs::File::create(&chromedriver_patched_name).unwrap();
        match std::fs::write(&chromedriver_patched_name, new_chromedriver_bytes) {
            Ok(_res) => {
                println!("Successfully wrote patched executable to '{chromedriver_patched_name}'!",)
            }
            Err(err) => println!("Error when writing patch to file! Error: {err}"),
        };
    }
    #[cfg(target_os = "linux")]
    {
        let mut perms = std::fs::metadata(&chromedriver_patched_name)
            .unwrap()
            .permissions();
        perms.set_mode(0o755);
        std::fs::set_permissions(&chromedriver_patched_name, perms).unwrap();
    }
    println!("Starting chromedriver...");
    let port: usize = rand::thread_rng().gen_range(2000..5000);
    Command::new(format!("./{}", &chromedriver_patched_name))
        .arg(format!("--port={}", port))
        .spawn()
        .expect("Failed to start chromedriver!");

    let mut caps = DesiredCapabilities::chrome();
    caps.set_no_sandbox().unwrap();
    caps.set_disable_dev_shm_usage().unwrap();
    caps.add_chrome_arg("--disable-blink-features=AutomationControlled").unwrap();
    caps.add_chrome_arg("window-size=1920,1080").unwrap();
    caps.add_chrome_arg("user-agent=Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/102.0.0.0 Safari/537.36").unwrap();
    caps.add_chrome_arg("disable-infobars").unwrap();
    caps.add_chrome_option("excludeSwitches", ["enable-automation"]).unwrap();

    let mut driver = None;
    let mut attempt = 0;
    while driver.is_none() && attempt < 20 {
        attempt += 1;
        match WebDriver::new(&format!("http://localhost:{}", port), caps.clone()).await {
            Ok(d) => driver = Some(d),
            Err(_) => std::thread::sleep(std::time::Duration::from_millis(250)),
        }
    }
    let driver = driver.unwrap();
    Ok(driver)
}

async fn fetch_chromedriver(client: &reqwest::Client) -> Result<(), Box<dyn std::error::Error>> {
    let os = std::env::consts::OS;
    let resp = client
        .get("https://chromedriver.storage.googleapis.com/LATEST_RELEASE")
        .send()
        .await?;
    let body = resp.text().await?;
    let url = format!("https://chromedriver.storage.googleapis.com/{body}/chromedriver_{}.zip", match os {
        "linux" => "linux64",
        "windows" => "win32",
        "macos" => "mac64",
        _ => panic!("Unsupported OS!")
    });
    
    let resp = client.get(url).send().await?;
    let body = resp.bytes().await?;

    let mut archive = zip::ZipArchive::new(std::io::Cursor::new(body))?;
    for i in 0..archive.len() {
        let mut file = archive.by_index(i)?;
        let outpath = file.mangled_name();
        if (&*file.name()).ends_with('/') {
            std::fs::create_dir_all(&outpath)?;
        } else {
            if let Some(p) = outpath.parent() {
                if !p.exists() {
                    std::fs::create_dir_all(&p)?;
                }
            }
            let mut outfile = std::fs::File::create(&outpath)?;
            std::io::copy(&mut file, &mut outfile)?;
        }
    }
    Ok(())
}

//! Seed catalog of banks (PLAN 2△ / banks): the worldwide directory the payment
//! form picks a sending bank and a receiving bank from. Names are English and
//! every bank resolves its own site favicon, so the list stays real without
//! shipping image assets. The admin endpoint can extend or disable any row.

use crate::banks::{repo_upsert, NewBank};
use crate::db::DbPool;

/// Bank logos come from the bank's own site favicon (Google favicon service).
pub fn icon_url(domain: &str) -> String {
    format!("https://www.google.com/s2/favicons?domain={domain}&sz=128")
}

/// Card schemes we attribute to a bank by its home market.
fn schemes_for(country: &str) -> &'static str {
    match country {
        "RU" => "Visa,MasterCard,MIR",
        "CN" => "UnionPay,Visa,MasterCard",
        "JP" => "Visa,MasterCard,JCB",
        "IN" => "Visa,MasterCard,RuPay",
        _ => "Visa,MasterCard",
    }
}

/// (name, country, currency, domain). Senders may be anywhere; every bank is
/// also a valid receiving bank, so the user composes the direction themselves.
const BANKS: &[(&str, &str, &str, &str)] = &[
    // --- CIS & nearby ---
    ("Sberbank", "RU", "RUB", "sberbank.ru"),
    ("Alfa-Bank", "RU", "RUB", "alfabank.ru"),
    ("VTB", "RU", "RUB", "vtb.ru"),
    ("T-Bank", "RU", "RUB", "tbank.ru"),
    ("Gazprombank", "RU", "RUB", "gazprombank.ru"),
    ("Rosbank", "RU", "RUB", "rosbank.ru"),
    ("MTS Bank", "RU", "RUB", "mtsbank.ru"),
    ("Raiffeisenbank Russia", "RU", "RUB", "raiffeisen.ru"),
    ("Sovcombank", "RU", "RUB", "sovcombank.ru"),
    ("Otkritie Bank", "RU", "RUB", "open.ru"),
    ("Russian Standard Bank", "RU", "RUB", "rsb.ru"),
    ("Uralsib Bank", "RU", "RUB", "uralsib.ru"),
    ("Post Bank", "RU", "RUB", "pochta.ru"),
    ("Home Credit Bank", "RU", "RUB", "homecredit.ru"),
    ("Yandex Bank", "RU", "RUB", "bank.yandex.ru"),
    ("Belarusbank", "BY", "BYN", "belarusbank.by"),
    ("Priorbank", "BY", "BYN", "priorbank.by"),
    ("MTBank", "BY", "BYN", "mtbank.by"),
    ("BPS-Sberbank", "BY", "BYN", "bps-sberbank.by"),
    ("Halyk Bank", "KZ", "KZT", "halykbank.kz"),
    ("Kaspi.kz", "KZ", "KZT", "kaspi.kz"),
    ("ForteBank", "KZ", "KZT", "fortebank.kz"),
    ("Jusan Bank", "KZ", "KZT", "jusan.kz"),
    ("Freedom Bank Kazakhstan", "KZ", "KZT", "freedombank.kz"),
    ("Bank CenterCredit", "KZ", "KZT", "centercredit.kz"),
    ("Bereke Bank", "KZ", "KZT", "berekebank.kz"),
    ("Eurasian Bank", "KZ", "KZT", "eubank.kz"),
    ("Kapitalbank", "UZ", "UZS", "kapitalbank.uz"),
    ("Uzpromstroybank", "UZ", "UZS", "uzpsb.uz"),
    ("Hamkorbank", "UZ", "UZS", "hamkorbank.uz"),
    ("Asakabank", "UZ", "UZS", "asakabank.uz"),
    ("Ipoteka Bank", "UZ", "UZS", "ipotekabank.uz"),
    ("TBC Bank", "GE", "GEL", "tbcbank.ge"),
    ("Bank of Georgia", "GE", "GEL", "bog.ge"),
    ("Liberty Bank", "GE", "GEL", "libertybank.ge"),
    ("Kapital Bank", "AZ", "AZN", "kapitalbank.az"),
    ("ABB Bank", "AZ", "AZN", "abb-bank.az"),
    ("Bank Respublika", "AZ", "AZN", "bankrespublika.az"),
    ("KICB", "KG", "KGS", "kicb.net"),
    ("Optima Bank", "KG", "KGS", "optimabank.kg"),
    ("MBank Kyrgyzstan", "KG", "KGS", "mbank.kg"),
    ("Ardshinbank", "AM", "AMD", "ardshinbank.am"),
    ("Ameriabank", "AM", "AMD", "ameriabank.am"),
    ("Acba Bank", "AM", "AMD", "acba.am"),
    ("VTB Armenia", "AM", "AMD", "vtb.am"),
    ("MAIB", "MD", "MDL", "maib.md"),
    ("Moldindconbank", "MD", "MDL", "micb.md"),
    ("Victoriabank", "MD", "MDL", "victoriabank.md"),
    ("Alif Bank", "TJ", "TJS", "alif.tj"),
    ("Eskhata Bank", "TJ", "TJS", "eskhata.tj"),
    ("Dushanbe City Bank", "TJ", "TJS", "dcbank.tj"),
    // --- North America ---
    ("JPMorgan Chase", "US", "USD", "jpmorganchase.com"),
    ("Bank of America", "US", "USD", "bankofamerica.com"),
    ("Wells Fargo", "US", "USD", "wellsfargo.com"),
    ("Citibank", "US", "USD", "citi.com"),
    ("Goldman Sachs", "US", "USD", "goldmansachs.com"),
    ("Morgan Stanley", "US", "USD", "morganstanley.com"),
    ("Capital One", "US", "USD", "capitalone.com"),
    ("U.S. Bank", "US", "USD", "usbank.com"),
    ("PNC Bank", "US", "USD", "pnc.com"),
    ("Truist", "US", "USD", "truist.com"),
    ("TD Bank US", "US", "USD", "td.com"),
    ("Charles Schwab", "US", "USD", "schwab.com"),
    ("American Express", "US", "USD", "americanexpress.com"),
    ("Discover", "US", "USD", "discover.com"),
    ("Ally Bank", "US", "USD", "ally.com"),
    ("Fifth Third Bank", "US", "USD", "53.com"),
    ("Regions Bank", "US", "USD", "regions.com"),
    ("KeyBank", "US", "USD", "key.com"),
    ("M&T Bank", "US", "USD", "mtb.com"),
    ("Huntington Bank", "US", "USD", "huntington.com"),
    ("RBC Royal Bank", "CA", "CAD", "rbc.com"),
    ("TD Canada Trust", "CA", "CAD", "td.com"),
    ("Scotiabank", "CA", "CAD", "scotiabank.com"),
    ("BMO Bank of Montreal", "CA", "CAD", "bmo.com"),
    ("CIBC", "CA", "CAD", "cibc.com"),
    ("National Bank of Canada", "CA", "CAD", "nbc.ca"),
    ("Desjardins", "CA", "CAD", "desjardins.com"),
    // --- United Kingdom ---
    ("HSBC UK", "GB", "GBP", "hsbc.co.uk"),
    ("Barclays", "GB", "GBP", "barclays.co.uk"),
    ("Lloyds Bank", "GB", "GBP", "lloydsbank.com"),
    ("NatWest", "GB", "GBP", "natwest.com"),
    ("Santander UK", "GB", "GBP", "santander.co.uk"),
    ("Halifax", "GB", "GBP", "halifax.co.uk"),
    ("Nationwide", "GB", "GBP", "nationwide.co.uk"),
    ("TSB Bank", "GB", "GBP", "tsb.co.uk"),
    ("Monzo", "GB", "GBP", "monzo.com"),
    ("Starling Bank", "GB", "GBP", "starlingbank.com"),
    ("Revolut", "GB", "GBP", "revolut.com"),
    ("Wise", "GB", "GBP", "wise.com"),
    // --- Germany ---
    ("Deutsche Bank", "DE", "EUR", "db.com"),
    ("Commerzbank", "DE", "EUR", "commerzbank.com"),
    ("DZ Bank", "DE", "EUR", "dzbank.de"),
    ("KfW", "DE", "EUR", "kfw.de"),
    ("Postbank", "DE", "EUR", "postbank.de"),
    ("HypoVereinsbank", "DE", "EUR", "hypovereinsbank.de"),
    ("ING Germany", "DE", "EUR", "ing.de"),
    ("N26", "DE", "EUR", "n26.com"),
    ("Sparkasse", "DE", "EUR", "sparkasse.de"),
    ("LBBW", "DE", "EUR", "lbbw.de"),
    ("BayernLB", "DE", "EUR", "bayernlb.de"),
    // --- France ---
    ("BNP Paribas", "FR", "EUR", "bnpparibas.com"),
    ("Societe Generale", "FR", "EUR", "societegenerale.com"),
    ("Credit Agricole", "FR", "EUR", "credit-agricole.com"),
    ("Credit Mutuel", "FR", "EUR", "creditmutuel.fr"),
    ("BPCE", "FR", "EUR", "bpce.com"),
    ("La Banque Postale", "FR", "EUR", "labanquepostale.fr"),
    ("LCL", "FR", "EUR", "lcl.fr"),
    ("CIC", "FR", "EUR", "cic.fr"),
    ("Boursorama", "FR", "EUR", "boursorama.com"),
    ("Hello bank", "FR", "EUR", "hellobank.fr"),
    // --- Switzerland ---
    ("UBS", "CH", "CHF", "ubs.com"),
    ("Julius Baer", "CH", "CHF", "juliusbaer.com"),
    ("Raiffeisen Switzerland", "CH", "CHF", "raiffeisen.ch"),
    ("Zuercher Kantonalbank", "CH", "CHF", "zkb.ch"),
    ("PostFinance", "CH", "CHF", "postfinance.ch"),
    ("Banque Cantonale Vaudoise", "CH", "CHF", "bcv.ch"),
    // --- Benelux ---
    ("ING", "NL", "EUR", "ing.com"),
    ("Rabobank", "NL", "EUR", "rabobank.com"),
    ("ABN AMRO", "NL", "EUR", "abnamro.com"),
    ("Triodos Bank", "NL", "EUR", "triodos.com"),
    ("bunq", "NL", "EUR", "bunq.com"),
    ("KBC", "BE", "EUR", "kbc.be"),
    ("Belfius", "BE", "EUR", "belfius.be"),
    ("BNP Paribas Fortis", "BE", "EUR", "bnpparibasfortis.be"),
    // --- Iberia ---
    ("Santander", "ES", "EUR", "santander.com"),
    ("BBVA", "ES", "EUR", "bbva.com"),
    ("CaixaBank", "ES", "EUR", "caixabank.com"),
    ("Bankinter", "ES", "EUR", "bankinter.com"),
    ("Banco Sabadell", "ES", "EUR", "bancsabadell.com"),
    ("Abanca", "ES", "EUR", "abanca.com"),
    ("Millennium BCP", "PT", "EUR", "millenniumbcp.pt"),
    ("Novo Banco", "PT", "EUR", "novobanco.pt"),
    // --- Italy ---
    ("UniCredit", "IT", "EUR", "unicreditgroup.eu"),
    ("Intesa Sanpaolo", "IT", "EUR", "intesasanpaolo.com"),
    ("Banco BPM", "IT", "EUR", "bancobpm.it"),
    ("BPER Banca", "IT", "EUR", "bper.it"),
    ("Mediobanca", "IT", "EUR", "mediobanca.com"),
    ("Credem", "IT", "EUR", "credem.it"),
    ("Banca Sella", "IT", "EUR", "sella.it"),
    // --- Nordics ---
    ("Nordea", "FI", "EUR", "nordea.com"),
    ("OP Financial Group", "FI", "EUR", "op.fi"),
    ("Danske Bank", "DK", "DKK", "danskebank.com"),
    ("Nykredit", "DK", "DKK", "nykredit.dk"),
    ("SEB", "SE", "SEK", "seb.se"),
    ("Swedbank", "SE", "SEK", "swedbank.com"),
    ("Handelsbanken", "SE", "SEK", "handelsbanken.com"),
    ("DNB", "NO", "NOK", "dnb.no"),
    ("SpareBank 1", "NO", "NOK", "sparebank1.no"),
    ("Lansforsakringar Bank", "SE", "SEK", "lansforsakringar.se"),
    ("Alandsbanken", "FI", "EUR", "alandsbanken.fi"),
    // --- Central & Eastern Europe ---
    ("Raiffeisen Bank International", "AT", "EUR", "rbinternational.com"),
    ("Erste Group", "AT", "EUR", "erstegroup.com"),
    ("Bank Austria", "AT", "EUR", "bankaustria.at"),
    ("Komercni banka", "CZ", "CZK", "kb.cz"),
    ("Moneta Money Bank", "CZ", "CZK", "moneta.cz"),
    ("Ceska sporitelna", "CZ", "CZK", "csas.cz"),
    ("PKO Bank Polski", "PL", "PLN", "pkobp.pl"),
    ("Bank Pekao", "PL", "PLN", "pekao.com.pl"),
    ("mBank", "PL", "PLN", "mbank.pl"),
    ("ING Bank Slaski", "PL", "PLN", "ing.pl"),
    ("OTP Bank", "HU", "HUF", "otpbank.hu"),
    ("K&H Bank", "HU", "HUF", "kh.hu"),
    ("Banca Transilvania", "RO", "RON", "bancatransilvania.ro"),
    ("Banca Comerciala Romana", "RO", "RON", "bcr.ro"),
    ("DSK Bank", "BG", "BGN", "dskbank.bg"),
    ("Eurobank", "GR", "EUR", "eurobank.gr"),
    ("Alpha Bank", "GR", "EUR", "alpha.gr"),
    ("Piraeus Bank", "GR", "EUR", "piraeusbank.gr"),
    ("National Bank of Greece", "GR", "EUR", "nbg.gr"),
    // --- Turkey ---
    ("Ziraat Bank", "TR", "TRY", "ziraatbank.com.tr"),
    ("Isbank", "TR", "TRY", "isbank.com.tr"),
    ("Garanti BBVA", "TR", "TRY", "garantibbva.com.tr"),
    ("Akbank", "TR", "TRY", "akbank.com"),
    ("Yapi Kredi", "TR", "TRY", "yapikredi.com.tr"),
    ("VakifBank", "TR", "TRY", "vakifbank.com.tr"),
    // --- Middle East ---
    ("Emirates NBD", "AE", "AED", "emiratesnbd.com"),
    ("First Abu Dhabi Bank", "AE", "AED", "bankfab.com"),
    ("Mashreq Bank", "AE", "AED", "mashreqbank.com"),
    ("Abu Dhabi Commercial Bank", "AE", "AED", "adcb.com"),
    ("Qatar National Bank", "QA", "QAR", "qnb.com"),
    ("Doha Bank", "QA", "QAR", "dohabank.com.qa"),
    ("Saudi National Bank", "SA", "SAR", "alahli.com"),
    ("Al Rajhi Bank", "SA", "SAR", "alrajhibank.com.sa"),
    ("Riyad Bank", "SA", "SAR", "riyadbank.com"),
    ("Saudi Awwal Bank", "SA", "SAR", "saib.com.sa"),
    ("Kuwait Finance House", "KW", "KWD", "kfh.com"),
    ("National Bank of Kuwait", "KW", "KWD", "nbk.com"),
    ("Bank Muscat", "OM", "OMR", "bankmuscat.com"),
    ("Arab Bank", "JO", "JOD", "arabbank.com"),
    ("Bank Leumi", "IL", "ILS", "leumi.co.il"),
    ("Bank Hapoalim", "IL", "ILS", "bankhapoalim.co.il"),
    ("Mizrahi-Tefahot Bank", "IL", "ILS", "mizrahi-tefahot.co.il"),
    // --- Asia ---
    ("DBS Bank", "SG", "SGD", "dbs.com"),
    ("OCBC Bank", "SG", "SGD", "ocbc.com"),
    ("United Overseas Bank", "SG", "SGD", "uob.com.sg"),
    ("Maybank", "MY", "MYR", "maybank.com"),
    ("CIMB Bank", "MY", "MYR", "cimb.com"),
    ("Public Bank", "MY", "MYR", "publicbank.com.my"),
    ("RHB Bank", "MY", "MYR", "rhbgroup.com"),
    ("Bank of China", "CN", "CNY", "boc.cn"),
    ("ICBC", "CN", "CNY", "icbc.com.cn"),
    ("China Construction Bank", "CN", "CNY", "ccb.com"),
    ("Agricultural Bank of China", "CN", "CNY", "abchina.com"),
    ("China Merchants Bank", "CN", "CNY", "cmbchina.com"),
    ("Bank of Communications", "CN", "CNY", "bankcomm.com"),
    ("MUFG Bank", "JP", "JPY", "mufg.jp"),
    ("SMBC", "JP", "JPY", "smbc.co.jp"),
    ("Mizuho Bank", "JP", "JPY", "mizuho.com"),
    ("Japan Post Bank", "JP", "JPY", "japanpost.jp"),
    ("Resona Bank", "JP", "JPY", "resona-gr.co.jp"),
    ("KB Kookmin Bank", "KR", "KRW", "kbstar.com"),
    ("Shinhan Bank", "KR", "KRW", "shinhan.com"),
    ("Hana Bank", "KR", "KRW", "hanabank.com"),
    ("Woori Bank", "KR", "KRW", "wooribank.com"),
    ("Kasikornbank", "TH", "THB", "kasikornbank.com"),
    ("Siam Commercial Bank", "TH", "THB", "scb.co.th"),
    ("Bangkok Bank", "TH", "THB", "bangkokbank.com"),
    ("BDO Unibank", "PH", "PHP", "bdo.com.ph"),
    ("Bank of the Philippine Islands", "PH", "PHP", "bpi.com.ph"),
    ("Bank Mandiri", "ID", "IDR", "bankmandiri.co.id"),
    ("Bank Central Asia", "ID", "IDR", "bca.co.id"),
    ("Bank Negara Indonesia", "ID", "IDR", "bni.co.id"),
    ("Bank Rakyat Indonesia", "ID", "IDR", "bri.co.id"),
    ("HDFC Bank", "IN", "INR", "hdfcbank.com"),
    ("ICICI Bank", "IN", "INR", "icicibank.com"),
    ("State Bank of India", "IN", "INR", "sbi.co.in"),
    ("Axis Bank", "IN", "INR", "axisbank.com"),
    ("Kotak Mahindra Bank", "IN", "INR", "kotak.com"),
    ("Punjab National Bank", "IN", "INR", "pnbindia.in"),
    // --- Oceania ---
    ("ANZ", "AU", "AUD", "anz.com"),
    ("Commonwealth Bank", "AU", "AUD", "commbank.com.au"),
    ("National Australia Bank", "AU", "AUD", "nab.com.au"),
    ("Westpac", "AU", "AUD", "westpac.com.au"),
    ("ASB Bank", "NZ", "NZD", "asb.co.nz"),
    ("Bank of New Zealand", "NZ", "NZD", "bnz.co.nz"),
    // --- Latin America ---
    ("Itau Unibanco", "BR", "BRL", "itau.com.br"),
    ("Bradesco", "BR", "BRL", "bradesco.com.br"),
    ("Banco do Brasil", "BR", "BRL", "bb.com.br"),
    ("Nubank", "BR", "BRL", "nubank.com.br"),
    ("Santander Brasil", "BR", "BRL", "santander.com.br"),
    ("BTG Pactual", "BR", "BRL", "btgpactual.com"),
    ("Banco Inter", "BR", "BRL", "bancointer.com.br"),
    ("Banco de Chile", "CL", "CLP", "bancochile.cl"),
    ("Banco de Credito e Inversiones", "CL", "CLP", "bci.cl"),
    ("Santander Chile", "CL", "CLP", "santander.cl"),
    ("Banco de Bogota", "CO", "COP", "bancodebogota.com"),
    ("Bancolombia", "CO", "COP", "bancolombia.com"),
    ("Davivienda", "CO", "COP", "davivienda.com"),
    ("BBVA Colombia", "CO", "COP", "bbva.com.co"),
    ("BBVA Mexico", "MX", "MXN", "bbva.mx"),
    ("Banorte", "MX", "MXN", "banorte.com"),
    ("Citibanamex", "MX", "MXN", "citibanamex.com"),
    ("BBVA Argentina", "AR", "ARS", "bbva.com.ar"),
    ("Banco Galicia", "AR", "ARS", "bancogalicia.com"),
    ("Banco de la Nacion Argentina", "AR", "ARS", "bna.com.ar"),
    ("Banco de Credito del Peru", "PE", "PEN", "viabcp.com"),
    ("Interbank", "PE", "PEN", "interbank.pe"),
    // --- Africa ---
    ("Standard Bank", "ZA", "ZAR", "standardbank.com"),
    ("First National Bank", "ZA", "ZAR", "fnb.co.za"),
    ("Absa", "ZA", "ZAR", "absa.africa"),
    ("Nedbank", "ZA", "ZAR", "nedbank.co.za"),
    ("Capitec Bank", "ZA", "ZAR", "capitec.co.za"),
    ("Access Bank", "NG", "NGN", "accessbankplc.com"),
    ("Guaranty Trust Bank", "NG", "NGN", "gtbank.com"),
    ("Zenith Bank", "NG", "NGN", "zenithbank.com"),
    ("First Bank of Nigeria", "NG", "NGN", "firstbanknigeria.com"),
    ("Equity Bank", "KE", "KES", "equitybank.co.ke"),
    ("KCB Bank", "KE", "KES", "kcbgroup.com"),
    ("National Bank of Egypt", "EG", "EGP", "nbe.com.eg"),
    ("Banque Misr", "EG", "EGP", "banquemisr.com"),
    ("Commercial International Bank", "EG", "EGP", "cibeg.com"),
    ("Attijariwafa Bank", "MA", "MAD", "attijariwafabank.com"),
    ("Bank of Africa", "MA", "MAD", "bankofafrica.ma"),
];

/// Build the bank directory as upsert payloads.
pub fn build_banks() -> Vec<NewBank> {
    BANKS
        .iter()
        .map(|(name, country, currency, domain)| NewBank {
            name: (*name).to_string(),
            role: Some("both".to_string()),
            country: Some((*country).to_string()),
            currency: Some((*currency).to_string()),
            domain: Some((*domain).to_string()),
            icon_url: Some(icon_url(domain)),
            schemes: Some(schemes_for(country).to_string()),
            status: Some("enabled".to_string()),
        })
        .collect()
}

/// Seed the `banks` table idempotently (upsert on `name`), called at startup.
pub async fn seed_banks(pool: &DbPool) -> anyhow::Result<usize> {
    let banks = build_banks();
    for bank in &banks {
        repo_upsert(pool, bank).await?;
    }
    Ok(banks.len())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    #[test]
    fn catalog_holds_at_least_two_hundred_worldwide_banks() {
        let banks = build_banks();
        assert!(banks.len() >= 200, "need >=200 banks, got {}", banks.len());
        let countries: HashSet<&str> = banks
            .iter()
            .filter_map(|b| b.country.as_deref())
            .collect();
        assert!(countries.len() >= 40, "banks should span the world, got {} countries", countries.len());
    }

    #[test]
    fn bank_names_are_english_and_unique() {
        let banks = build_banks();
        let mut seen = HashSet::new();
        for bank in &banks {
            assert!(
                bank.name.is_ascii(),
                "bank name must be latin/english, got {}",
                bank.name
            );
            assert!(seen.insert(bank.name.clone()), "duplicate bank name {}", bank.name);
        }
    }

    #[test]
    fn every_bank_has_a_favicon_and_schemes() {
        for bank in build_banks() {
            assert!(bank.icon_url.as_deref().unwrap_or("").contains("favicons"));
            assert!(!bank.schemes.as_deref().unwrap_or("").is_empty());
        }
    }
}

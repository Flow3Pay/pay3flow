//! Seed catalog of payment methods (PLAN 2△ / banks): the worldwide directory
//! the swap form picks a sending method and a receiving method from. Names are
//! English and every method resolves its own site favicon, so the list stays
//! real without shipping image assets. The admin endpoint can extend or
//! disable any row.

use crate::banks::{repo_upsert, NewBank};
use crate::db::DbPool;

/// Method logos come from the provider's own site favicon (Google favicon service).
pub fn icon_url(domain: &str) -> String {
    format!("https://www.google.com/s2/favicons?domain_url=https://{domain}/&sz=128")
}

fn branded_icon_url(name: &str, domain: &str) -> String {
    match name {
        "Visa Network" => {
            "https://upload.wikimedia.org/wikipedia/commons/9/98/Visa_Inc._logo_%282005%E2%80%932014%29.svg?utm_source=commons.wikimedia.org&utm_campaign=index&utm_content=original".to_string()
        }
        "Mastercard Network" => {
            "https://thumb.wikimedia.org/wikipedia/commons/thumb/a/a4/Mastercard_2019_logo.svg/1280px-Mastercard_2019_logo.svg.png?utm_source=en.wikipedia.org&utm_campaign=index&utm_content=thumbnail".to_string()
        }
        "MIR Network" => "https://evgenykatyshev.ru/projects/mir-logo/mir-logo.svg".to_string(),
        "PayPal" => {
            "https://thumb.wikimedia.org/wikipedia/commons/thumb/0/0e/PayPal_2024_%28Icon%29.svg/250px-PayPal_2024_%28Icon%29.svg.png?utm_source=en.wikipedia.org&utm_campaign=parser&utm_content=thumbnail".to_string()
        }
        _ => icon_url(domain),
    }
}

/// Card schemes we attribute to a bank by its home market.
fn schemes_for(country: &str) -> &'static str {
    match country {
        "BY" => "Visa,MasterCard,MIR",
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

/// (name, country, currency, domain, rail). Non-bank payment methods and
/// payment networks live in the same catalog so the swap picker can offer cards,
/// wallets, local rails and mobile money next to bank accounts.
const PAYMENT_METHODS: &[(&str, &str, &str, &str, &str)] = &[
    // --- Global card and account rails ---
    ("Visa Network", "GLOBAL", "USD", "visa.com", "Visa"),
    ("Mastercard Network", "GLOBAL", "USD", "mastercard.com", "MasterCard"),
    ("MIR Network", "RU", "RUB", "mironline.ru", "MIR"),
    ("UnionPay Network", "CN", "CNY", "unionpayintl.com", "UnionPay"),
    ("JCB Network", "JP", "JPY", "global.jcb", "JCB"),
    ("American Express Network", "US", "USD", "americanexpress.com", "AmEx"),
    ("Discover Network", "US", "USD", "discover.com", "Discover"),
    ("Diners Club", "US", "USD", "dinersclub.com", "Diners Club"),
    ("RuPay Network", "IN", "INR", "rupay.co.in", "RuPay"),
    ("Verve", "NG", "NGN", "myverveworld.com", "Verve"),
    // --- Global wallets and money apps ---
    ("PayPal", "GLOBAL", "USD", "paypal.com", "PayPal"),
    ("Venmo", "US", "USD", "venmo.com", "Venmo"),
    ("Cash App", "US", "USD", "cash.app", "Cash App"),
    ("Apple Pay", "GLOBAL", "USD", "apple.com", "Apple Pay"),
    ("Google Pay", "GLOBAL", "USD", "pay.google.com", "Google Pay"),
    ("Samsung Wallet", "GLOBAL", "USD", "samsung.com", "Samsung Wallet"),
    ("Skrill", "GLOBAL", "EUR", "skrill.com", "Skrill"),
    ("Neteller", "GLOBAL", "EUR", "neteller.com", "Neteller"),
    ("Payoneer Account", "GLOBAL", "USD", "payoneer.com", "Payoneer"),
    ("Wise Account", "GLOBAL", "USD", "wise.com", "Wise"),
    ("Revolut Wallet", "GLOBAL", "EUR", "revolut.com", "Revolut"),
    ("Paysend", "GLOBAL", "USD", "paysend.com", "Paysend"),
    ("Remitly", "GLOBAL", "USD", "remitly.com", "Remitly"),
    ("Western Union", "GLOBAL", "USD", "westernunion.com", "Western Union"),
    ("MoneyGram", "GLOBAL", "USD", "moneygram.com", "MoneyGram"),
    ("Zelle", "US", "USD", "zellepay.com", "Zelle"),
    ("Stripe Link", "GLOBAL", "USD", "link.com", "Link"),
    ("Klarna", "GLOBAL", "EUR", "klarna.com", "Klarna"),
    ("Afterpay", "AU", "AUD", "afterpay.com", "Afterpay"),
    // --- CIS and Eastern Europe wallets/rails ---
    ("YooMoney", "RU", "RUB", "yoomoney.ru", "YooMoney"),
    ("QIWI Wallet", "RU", "RUB", "qiwi.com", "QIWI"),
    ("SBP Fast Payments", "RU", "RUB", "sbp.nspk.ru", "SBP"),
    ("SberPay", "RU", "RUB", "sberbank.ru", "SberPay"),
    ("T-Pay", "RU", "RUB", "tbank.ru", "T-Pay"),
    ("MTS Money", "RU", "RUB", "mtsbank.ru", "MTS Money"),
    ("ERIP Raschet", "BY", "BYN", "raschet.by", "ERIP"),
    ("O!Pay", "KG", "KGS", "opay.kg", "O!Pay"),
    ("Kaspi Pay", "KZ", "KZT", "kaspi.kz", "Kaspi Pay"),
    ("Halyk QR", "KZ", "KZT", "halykbank.kz", "Halyk QR"),
    ("Click Uzbekistan", "UZ", "UZS", "click.uz", "Click"),
    ("Payme Uzbekistan", "UZ", "UZS", "payme.uz", "Payme"),
    ("Uzcard", "UZ", "UZS", "uzcard.uz", "Uzcard"),
    ("Humo", "UZ", "UZS", "humocard.uz", "Humo"),
    ("ArCa", "AM", "AMD", "arca.am", "ArCa"),
    ("Idram", "AM", "AMD", "idram.am", "Idram"),
    ("EasyPay Georgia", "GE", "GEL", "easypay.ge", "EasyPay"),
    // --- Europe local rails ---
    ("SEPA Transfer", "EU", "EUR", "europeanpaymentscouncil.eu", "SEPA"),
    ("SEPA Instant", "EU", "EUR", "europeanpaymentscouncil.eu", "SEPA Instant"),
    ("Sofort", "DE", "EUR", "sofort.com", "Sofort"),
    ("Giropay", "DE", "EUR", "giropay.de", "Giropay"),
    ("iDEAL", "NL", "EUR", "ideal.nl", "iDEAL"),
    ("Bancontact", "BE", "EUR", "bancontact.com", "Bancontact"),
    ("BLIK", "PL", "PLN", "blik.com", "BLIK"),
    ("Przelewy24", "PL", "PLN", "przelewy24.pl", "Przelewy24"),
    ("Swish", "SE", "SEK", "swish.nu", "Swish"),
    ("Vipps", "NO", "NOK", "vipps.no", "Vipps"),
    ("MobilePay", "DK", "DKK", "mobilepay.dk", "MobilePay"),
    ("Twint", "CH", "CHF", "twint.ch", "Twint"),
    ("Paylib", "FR", "EUR", "paylib.fr", "Paylib"),
    ("Bizum", "ES", "EUR", "bizum.es", "Bizum"),
    ("Satispay", "IT", "EUR", "satispay.com", "Satispay"),
    // --- Asia wallets and rails ---
    ("Alipay", "CN", "CNY", "alipay.com", "Alipay"),
    ("WeChat Pay", "CN", "CNY", "wechatpay.com", "WeChat Pay"),
    ("Octopus", "HK", "HKD", "octopus.com.hk", "Octopus"),
    ("PayPay Japan", "JP", "JPY", "paypay.ne.jp", "PayPay"),
    ("Rakuten Pay", "JP", "JPY", "pay.rakuten.co.jp", "Rakuten Pay"),
    ("LINE Pay", "JP", "JPY", "linepay.line.me", "LINE Pay"),
    ("Kakao Pay", "KR", "KRW", "kakaopay.com", "Kakao Pay"),
    ("Naver Pay", "KR", "KRW", "pay.naver.com", "Naver Pay"),
    ("Toss", "KR", "KRW", "toss.im", "Toss"),
    ("Paytm", "IN", "INR", "paytm.com", "Paytm"),
    ("PhonePe", "IN", "INR", "phonepe.com", "PhonePe"),
    ("UPI", "IN", "INR", "npci.org.in", "UPI"),
    ("BHIM", "IN", "INR", "bhimupi.org.in", "BHIM"),
    ("GCash", "PH", "PHP", "gcash.com", "GCash"),
    ("Maya Philippines", "PH", "PHP", "maya.ph", "Maya"),
    ("GrabPay", "SG", "SGD", "grab.com", "GrabPay"),
    ("ShopeePay", "SG", "SGD", "shopeepay.com", "ShopeePay"),
    ("Touch n Go eWallet", "MY", "MYR", "touchngo.com.my", "Touch n Go"),
    ("DuitNow", "MY", "MYR", "duitnow.my", "DuitNow"),
    ("TrueMoney", "TH", "THB", "truemoney.com", "TrueMoney"),
    ("PromptPay", "TH", "THB", "bot.or.th", "PromptPay"),
    ("MoMo Vietnam", "VN", "VND", "momo.vn", "MoMo"),
    ("ZaloPay", "VN", "VND", "zalopay.vn", "ZaloPay"),
    ("OVO", "ID", "IDR", "ovo.id", "OVO"),
    ("DANA", "ID", "IDR", "dana.id", "DANA"),
    ("GoPay", "ID", "IDR", "gopay.co.id", "GoPay"),
    // --- Middle East, Africa, LatAm rails ---
    ("STC Pay", "SA", "SAR", "stcpay.com.sa", "STC Pay"),
    ("Fawry", "EG", "EGP", "fawry.com", "Fawry"),
    ("M-Pesa", "KE", "KES", "mpesa.com", "M-Pesa"),
    ("Airtel Money", "GLOBAL", "USD", "airtel.africa", "Airtel Money"),
    ("Orange Money", "GLOBAL", "EUR", "orange.com", "Orange Money"),
    ("MTN Mobile Money", "GLOBAL", "USD", "mtn.com", "MTN MoMo"),
    ("Pix", "BR", "BRL", "bcb.gov.br", "PIX"),
    ("Boleto Bancario", "BR", "BRL", "febraban.org.br", "Boleto"),
    ("Mercado Pago", "LATAM", "USD", "mercadopago.com", "Mercado Pago"),
    ("SPEI", "MX", "MXN", "banxico.org.mx", "SPEI"),
    ("OXXO Pay", "MX", "MXN", "oxxo.com", "OXXO Pay"),
    ("PSE Colombia", "CO", "COP", "pse.com.co", "PSE"),
    ("Webpay", "CL", "CLP", "transbank.cl", "Webpay"),
    ("PagoEfectivo", "PE", "PEN", "pagoefectivo.pe", "PagoEfectivo"),
    // --- Crypto rails shown as payment methods ---
    ("Bitcoin Network", "GLOBAL", "BTC", "bitcoin.org", "Bitcoin"),
    ("Ethereum Network", "GLOBAL", "ETH", "ethereum.org", "Ethereum"),
    ("TON Network", "GLOBAL", "TON", "ton.org", "TON"),
    ("Tether USDT TRC20", "GLOBAL", "USDT", "tether.to", "USDT TRC20"),
    ("Tether USDT ERC20", "GLOBAL", "USDT", "tether.to", "USDT ERC20"),
    ("USD Coin", "GLOBAL", "USDC", "circle.com", "USDC"),
];

/// Build the payment-method directory as upsert payloads.
pub fn build_banks() -> Vec<NewBank> {
    let mut entries = BANKS
        .iter()
        .flat_map(|(name, country, currency, domain)| {
            schemes_for(country)
                .split(',')
                .map(move |scheme| {
                    (
                        format!("{name} {scheme}"),
                        *country,
                        *currency,
                        *domain,
                        scheme.to_string(),
                    )
                })
        })
        .chain(PAYMENT_METHODS.iter().map(|(name, country, currency, domain, scheme)| {
            (
                (*name).to_string(),
                *country,
                *currency,
                *domain,
                (*scheme).to_string(),
            )
        }))
        .map(|(name, country, currency, domain, schemes)| {
            let icon_url = branded_icon_url(&name, domain);
            NewBank {
                name,
                role: Some("both".to_string()),
                country: Some(country.to_string()),
                currency: Some(currency.to_string()),
                domain: Some(domain.to_string()),
                icon_url: Some(icon_url),
                schemes: Some(schemes),
                status: Some("enabled".to_string()),
            }
        })
        .collect::<Vec<_>>();
    entries.sort_by(|left, right| left.name.cmp(&right.name));
    entries
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
    fn catalog_holds_many_worldwide_payment_methods() {
        let banks = build_banks();
        assert!(banks.len() >= 500, "need >=500 methods, got {}", banks.len());
        let countries: HashSet<&str> = banks
            .iter()
            .filter_map(|b| b.country.as_deref())
            .collect();
        assert!(countries.len() >= 40, "methods should span the world, got {} countries", countries.len());
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
    fn every_method_has_a_favicon_and_schemes() {
        for bank in build_banks() {
            assert!(bank.icon_url.as_deref().unwrap_or("").starts_with("https://"));
            assert!(!bank.schemes.as_deref().unwrap_or("").is_empty());
        }
    }

    #[test]
    fn major_standalone_methods_use_their_brand_logos() {
        let banks = build_banks();
        let icon_for = |name: &str| {
            banks
                .iter()
                .find(|bank| bank.name == name)
                .and_then(|bank| bank.icon_url.as_deref())
                .unwrap_or("")
        };

        assert!(icon_for("Visa Network").contains("Visa_Inc._logo"));
        assert!(icon_for("Mastercard Network").contains("Mastercard_2019_logo"));
        assert!(icon_for("MIR Network").contains("mir-logo.svg"));
        assert!(icon_for("PayPal").contains("PayPal_2024"));
    }

    #[test]
    fn catalog_includes_wallets_networks_and_local_rails() {
        let names = build_banks()
            .into_iter()
            .map(|b| b.name)
            .collect::<HashSet<_>>();
        for expected in [
            "Alfa-Bank Visa",
            "Belarusbank MasterCard",
            "Belarusbank MIR",
            "PayPal",
            "YooMoney",
            "Visa Network",
            "MIR Network",
            "SEPA Instant",
            "Pix",
            "UPI",
        ] {
            assert!(names.contains(expected), "missing payment method {expected}");
        }
    }

    #[test]
    fn bank_card_schemes_are_individual_selectable_methods() {
        let banks = build_banks();
        let alfa_visa = banks
            .iter()
            .find(|bank| bank.name == "Alfa-Bank Visa")
            .expect("Alfa-Bank Visa must be selectable");
        let alfa_mastercard = banks
            .iter()
            .find(|bank| bank.name == "Alfa-Bank MasterCard")
            .expect("Alfa-Bank MasterCard must be selectable");
        let belarusbank_mir = banks
            .iter()
            .find(|bank| bank.name == "Belarusbank MIR")
            .expect("Belarusbank MIR must be selectable");

        assert_eq!(alfa_visa.schemes.as_deref(), Some("Visa"));
        assert_eq!(alfa_mastercard.schemes.as_deref(), Some("MasterCard"));
        assert_eq!(belarusbank_mir.schemes.as_deref(), Some("MIR"));
        assert_eq!(alfa_visa.icon_url, alfa_mastercard.icon_url);
    }
}

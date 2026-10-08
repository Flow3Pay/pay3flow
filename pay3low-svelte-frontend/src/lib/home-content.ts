import type { Locale } from "./i18n";

export const SITE_URL = "https://pay3flow.lefine.pro/";
export const PROJECT_URL = "https://github.com/Flow3Pay/pay3flow";
export const COMMUNITY_URL = "https://t.me/+-lq4m5E_aT4xM2Y6";

type HomeContent = {
  title: string;
  description: string;
  intro: string;
  nojs: string;
  aboutTitle: string;
  about: string;
  howTitle: string;
  steps: string[];
  coverageTitle: string;
  currencies: string;
  sources: string;
  examplesTitle: string;
  examples: { route: string; explanation: string; provider?: string; providerName?: string }[];
  methodTitle: string;
  method: string[];
  faqTitle: string;
  faq: { question: string; answer: string }[];
  projectTitle: string;
  project: string;
  github: string;
  telegram: string;
  terms: string;
};

export const homeContent: Record<Locale, HomeContent> = {
  ru: {
    title: "Pay3Flow — сравнение маршрутов обмена валют и криптовалют",
    description: "Сравните маршруты обмена валют и криптовалют через P2P, обменники и спот. Pay3Flow показывает расчётную сумму получения, шаги обмена и условия площадок.",
    intro: "Не тратьте своё время на поиск обмена.",
    nojs: "Описание сервиса и ответы на вопросы доступны без JavaScript. Для загрузки котировок и поиска маршрутов включите JavaScript в браузере.",
    aboutTitle: "Что такое Pay3Flow",
    about: "Pay3Flow — экспериментальный сервис с открытым исходным кодом для сравнения маршрутов обмена. Он сопоставляет публичные P2P-объявления, предложения обменников и цены спотовых рынков. Вы можете сравнить прямой обмен с маршрутом через промежуточный цифровой актив и увидеть, сколько примерно получите на каждом шаге.",
    howTitle: "Как найти маршрут",
    steps: [
      "Укажите исходную и конечную валюту или цифровой актив, способ оплаты и сумму. Для криптовалют выберите сеть.",
      "Выберите площадки для сравнения. Предложения появляются по мере получения ответов, а маршруты упорядочиваются по расчётному результату и условиям обмена.",
      "Откройте инструкцию выбранного маршрута, изучите его шаги и проверьте актуальные условия на площадках перед обменом.",
    ],
    coverageTitle: "Валюты, активы и источники",
    currencies: "В каталоге есть армянский драм (AMD), российский рубль (RUB), доллар США (USD), белорусский рубль (BYN) и казахстанский тенге (KZT), а также USDT, USDC, BTC и ETH. Доступность конкретного направления зависит от способа оплаты, сети, лимитов и текущих предложений площадок.",
    sources: "Среди подключённых источников — Binance, Bybit, OKX, Bitget и MEXC для P2P и спотовых рынков; Rapira, Whitebird, Cifra Markets, SkyLabs, bncex и Bitcoin Center для отдельных направлений. Источники и доступные способы обмена можно посмотреть в настройках поиска. Список не означает, что каждая площадка поддерживает все перечисленные валюты.",
    examplesTitle: "Примеры маршрутов",
    examples: [
      { route: "AMD → USDT → RUB", explanation: "Обмен драмов на рубли через промежуточный цифровой актив." },
      { route: "USDC (Ethereum) → RUB", explanation: "Сравнение выхода из цифрового актива в рубли через доступные способы получения." },
      { route: "USD cash → USDT → AMD", explanation: "Поиск маршрута из наличных долларов в драмы, когда площадки предлагают подходящие условия." },
      { route: "ETH (Ethereum) → USDC (Ethereum)", explanation: "Обмен ETH на USDC в сети Ethereum через CoW Swap.", provider: "cow-swap", providerName: "CoW Swap" },
      { route: "USDT (Ethereum) → USDC (Solana)", explanation: "Обмен USDT в Ethereum на USDC в Solana через Symbiosis.", provider: "symbiosis", providerName: "Symbiosis" },
      { route: "BTC → ETH (Ethereum)", explanation: "Обмен BTC на ETH через NEAR Intents.", provider: "near-intents", providerName: "NEAR Intents" },
    ],
    methodTitle: "Как считаются и сравниваются предложения",
    method: [
      "При поиске Pay3Flow запрашивает выбранные источники параллельно; часть ответов может поступать из кеша. Для обычного обмена сравнивается расчётная сумма в конечной валюте, для циклического — также расчётный прирост. В порядке отображения учитываются подтверждённое соответствие способу оплаты, использование одной площадки и отзывы пользователей при близких суммах. Это сравнение расчётов, а не оценка надёжности продавца или гарантия исполнения.",
      "В расчёте отражаются комиссии, которые источник передаёт в котировке, и оценки, указанные в условиях маршрута. Для спотовых шагов может использоваться оценочная торговая комиссия. Не все расходы известны заранее: комиссии банка, перевода между площадками, вывода, газа и особенности вашего аккаунта могут изменить итог. Проверьте предупреждения, лимиты, сеть и окончательную сумму у каждого участника обмена.",
    ],
    faqTitle: "Частые вопросы",
    faq: [
      { question: "Почему сумма расчётная?", answer: "Курс, доступный объём и лимиты могут измениться между поиском и созданием заявки на площадке. Котировка не резервирует предложение. Обновите поиск и подтвердите условия непосредственно перед обменом." },
      { question: "Pay3Flow хранит деньги или создаёт P2P-заявку?", answer: "Поиск маршрутов не принимает ваши деньги, не создаёт P2P-заявки и не связывается с продавцом. Если отдельная функция предлагает действие через подключённый кошелёк, проверьте параметры и запрос на подпись перед подтверждением." },
      { question: "Почему нет подходящего маршрута?", answer: "Для выбранной суммы, валют, сети или способа оплаты может не быть предложений в нужных лимитах. Источник также может временно не отвечать. Попробуйте другую сумму, набор площадок или способ оплаты и запустите поиск снова." },
    ],
    projectTitle: "О проекте и обратная связь",
    project: "Pay3Flow развивается как открытый проект Flow3Pay на GitHub. Там можно изучить исходный код, методику расчётов и сообщить об ошибке через Issues. Обсуждение проекта доступно в Telegram; правила использования описывают возможности и ограничения сервиса.",
    github: "Исходный код и сообщения об ошибках",
    telegram: "Сообщество в Telegram",
    terms: "Правила использования",
  },
  en: {
    title: "Pay3Flow — compare currency and crypto exchange routes",
    description: "Compare currency and crypto exchange routes across P2P markets, exchangers and spot markets. See estimated amounts, exchange steps and provider conditions with Pay3Flow.",
    intro: "Don’t waste your time looking for an exchange.",
    nojs: "Product information and answers are available without JavaScript. Enable JavaScript in your browser to load quotes and search for routes.",
    aboutTitle: "What is Pay3Flow?",
    about: "Pay3Flow is an experimental open source service for comparing exchange routes. It brings together public P2P advertisements, exchanger quotes and spot market prices. Compare a direct exchange with a route through an intermediate digital asset and see the estimated amount at each step.",
    howTitle: "How to find a route",
    steps: [
      "Choose your source and destination currency or digital asset, payment method and amount. Select a network for crypto assets.",
      "Choose the providers to compare. Offers appear as sources respond; routes are ordered by estimated results and exchange conditions.",
      "Open the instructions for a route, review each step and confirm current conditions with the providers before exchanging.",
    ],
    coverageTitle: "Currencies, assets and sources",
    currencies: "The catalog includes Armenian dram (AMD), Russian ruble (RUB), US dollar (USD), Belarusian ruble (BYN) and Kazakhstani tenge (KZT), alongside USDT, USDC, BTC and ETH. Availability depends on the payment method, network, limits and current provider offers.",
    sources: "Connected sources include Binance, Bybit, OKX, Bitget and MEXC for P2P and spot markets; Rapira, Whitebird, Cifra Markets, SkyLabs, bncex and Bitcoin Center for specific routes. Search settings show sources and exchange methods. Each provider supports its own subset of currencies.",
    examplesTitle: "Example routes",
    examples: [
      { route: "AMD → USDT → RUB", explanation: "Exchange dram for rubles through an intermediate digital asset." },
      { route: "USDC (Ethereum) → RUB", explanation: "Compare ways to exchange a digital asset for rubles using available payout methods." },
      { route: "USD cash → USDT → AMD", explanation: "Find a route from cash dollars to dram when matching provider offers are available." },
      { route: "ETH (Ethereum) → USDC (Ethereum)", explanation: "Swap ETH for USDC on Ethereum through CoW Swap.", provider: "cow-swap", providerName: "CoW Swap" },
      { route: "USDT (Ethereum) → USDC (Solana)", explanation: "Swap USDT on Ethereum for USDC on Solana through Symbiosis.", provider: "symbiosis", providerName: "Symbiosis" },
      { route: "BTC → ETH (Ethereum)", explanation: "Exchange BTC for ETH through NEAR Intents.", provider: "near-intents", providerName: "NEAR Intents" },
    ],
    methodTitle: "How offers are calculated and compared",
    method: [
      "Pay3Flow queries selected sources in parallel; some responses may be cached. Ordinary exchanges compare estimated destination amounts; cycles also compare estimated gains. Ordering considers verified payment method matches, use of one venue and user feedback when amounts are close. This compares estimates; it does not rate advertiser trustworthiness or guarantee execution.",
      "Estimates reflect fees returned in provider quotes and estimates described in route conditions. Spot steps may use an estimated trading fee. Not every cost is known: bank charges, cross-venue transfers, withdrawals, gas and account conditions can change the final amount. Check warnings, limits, networks and final amounts at each provider.",
    ],
    faqTitle: "Frequently asked questions",
    faq: [
      { question: "Why is the amount an estimate?", answer: "Rates, available volume and limits may change before you create an order. A quote does not reserve an offer. Refresh the search and confirm conditions immediately before exchanging." },
      { question: "Does Pay3Flow hold funds or create P2P orders?", answer: "Route search does not accept your money, create P2P orders or contact advertisers. If a separate feature offers an action through a connected wallet, review the parameters and signature request before confirming." },
      { question: "Why are there no matching routes?", answer: "There may be no offers matching your amount, currencies, network or payment method within available limits. A source may also be temporarily unavailable. Try a different amount, provider selection or payment method and search again." },
    ],
    projectTitle: "About the project and feedback",
    project: "Pay3Flow is developed as the open source Flow3Pay project on GitHub. Explore the code and calculation methods there, or report a bug through Issues. The project also has a Telegram community. The usage policy explains the service's features and limitations.",
    github: "Source code and bug reports",
    telegram: "Telegram community",
    terms: "Usage policy",
  },
  hy: {
    title: "Pay3Flow — համեմատեք արժույթի և կրիպտոարժույթի փոխանակման ուղիները",
    description: "Համեմատեք արժույթի և կրիպտոարժույթի փոխանակման ուղիները P2P հարթակներում, փոխանակման ծառայություններում և սփոթ շուկաներում։ Տեսեք մոտավոր գումարներն ու քայլերը։",
    intro: "Մի՛ վատնեք ձեր ժամանակը փոխանակում փնտրելու վրա։",
    nojs: "Ծառայության նկարագրությունն ու պատասխանները հասանելի են առանց JavaScript-ի։ Հաշվարկները բեռնելու և ուղիներ որոնելու համար միացրեք JavaScript-ը։",
    aboutTitle: "Ի՞նչ է Pay3Flow-ը",
    about: "Pay3Flow-ը բաց կոդով փորձարարական ծառայություն է փոխանակման ուղիները համեմատելու համար։ Այն համադրում է հրապարակային P2P հայտարարությունները, փոխանակման ծառայությունների առաջարկներն ու սփոթ գները։ Համեմատեք ուղղակի փոխանակումը միջանկյալ թվային ակտիվով ուղու հետ և տեսեք յուրաքանչյուր քայլի մոտավոր գումարը։",
    howTitle: "Ինչպես գտնել ուղին",
    steps: [
      "Ընտրեք սկզբնական և վերջնական արժույթը կամ թվային ակտիվը, վճարման եղանակն ու գումարը։ Կրիպտոարժույթի համար ընտրեք ցանցը։",
      "Ընտրեք համեմատվող հարթակները։ Առաջարկները հայտնվում են պատասխանները ստանալուն պես։ Ուղիները դասավորվում են ըստ հաշվարկային արդյունքի և փոխանակման պայմանների։",
      "Բացեք ուղու հրահանգները, ուսումնասիրեք քայլերը և փոխանակումից առաջ հաստատեք պայմանները հարթակներում։",
    ],
    coverageTitle: "Արժույթներ, ակտիվներ և աղբյուրներ",
    currencies: "Կատալոգում կան հայկական դրամ (AMD), ռուսական ռուբլի (RUB), ԱՄՆ դոլար (USD), բելառուսական ռուբլի (BYN), ղազախական տենգե (KZT), ինչպես նաև USDT, USDC, BTC և ETH։ Հասանելիությունը կախված է վճարման եղանակից, ցանցից, սահմանաչափերից և ընթացիկ առաջարկներից։",
    sources: "Միացված աղբյուրներից են Binance, Bybit, OKX, Bitget և MEXC՝ P2P և սփոթ շուկաների համար, ինչպես նաև Rapira, Whitebird, Cifra Markets, SkyLabs, bncex և Bitcoin Center՝ առանձին ուղղությունների համար։ Յուրաքանչյուր հարթակ աջակցում է արժույթների իր ցանկին։ Աղբյուրները կարելի է տեսնել որոնման կարգավորումներում։",
    examplesTitle: "Ուղիների օրինակներ",
    examples: [
      { route: "AMD → USDT → RUB", explanation: "Դրամի փոխանակում ռուբլու՝ միջանկյալ թվային ակտիվի միջոցով։" },
      { route: "USDC (Ethereum) → RUB", explanation: "Թվային ակտիվից ռուբլու անցման եղանակների համեմատություն։" },
      { route: "USD cash → USDT → AMD", explanation: "Կանխիկ դոլարից դրամի ուղու որոնում՝ համապատասխան առաջարկների առկայության դեպքում։" },
      { route: "ETH (Ethereum) → USDC (Ethereum)", explanation: "ETH-ի փոխանակում USDC-ի Ethereum ցանցում՝ CoW Swap-ի միջոցով։", provider: "cow-swap", providerName: "CoW Swap" },
      { route: "USDT (Ethereum) → USDC (Solana)", explanation: "Ethereum ցանցի USDT-ի փոխանակում Solana ցանցի USDC-ի՝ Symbiosis-ի միջոցով։", provider: "symbiosis", providerName: "Symbiosis" },
      { route: "BTC → ETH (Ethereum)", explanation: "BTC-ի փոխանակում ETH-ի՝ NEAR Intents-ի միջոցով։", provider: "near-intents", providerName: "NEAR Intents" },
    ],
    methodTitle: "Ինչպես են համեմատվում առաջարկները",
    method: [
      "Pay3Flow-ը հարցումներ է ուղարկում ընտրված աղբյուրներին զուգահեռ։ Որոշ պատասխաններ կարող են լինել քեշից։ Համեմատվում է վերջնական մոտավոր գումարը, իսկ ցիկլերի համար՝ նաև մոտավոր աճը։ Դասավորումը հաշվի է առնում վճարման եղանակի հաստատված համապատասխանությունը, մեկ հարթակի օգտագործումը և մոտ գումարների դեպքում՝ օգտատերերի արձագանքները։ Սա վաճառողի հուսալիության գնահատական կամ կատարման երաշխիք չէ։",
      "Հաշվարկում արտացոլվում են աղբյուրի տրամադրած միջնորդավճարները և ուղու պայմաններում նշված գնահատականները։ Սփոթ քայլերում կարող է կիրառվել գնահատված առևտրային վճար։ Բանկի, փոխանցման, դուրսբերման, գազի կամ ձեր հաշվի ծախսերը կարող են փոխել արդյունքը։ Ստուգեք սահմանաչափերը, ցանցերն ու վերջնական գումարը հարթակներում։",
    ],
    faqTitle: "Հաճախ տրվող հարցեր",
    faq: [
      { question: "Ինչո՞ւ է գումարը մոտավոր", answer: "Փոխարժեքը, ծավալն ու սահմանաչափերը կարող են փոխվել մինչև պատվերի ստեղծումը։ Հաշվարկը առաջարկ չի ամրագրում։ Թարմացրեք որոնումը և հաստատեք պայմանները փոխանակումից առաջ։" },
      { question: "Pay3Flow-ը պահո՞ւմ է գումարը կամ ստեղծո՞ւմ է P2P պատվեր", answer: "Ուղիների որոնումը գումար չի ընդունում, P2P պատվերներ չի ստեղծում և վաճառողի հետ չի կապվում։ Եթե առանձին գործառույթ առաջարկում է գործողություն միացված դրամապանակով, հաստատումից առաջ ստուգեք պարամետրերն ու ստորագրության հարցումը։" },
      { question: "Ինչո՞ւ համապատասխան ուղի չկա", answer: "Ընտրված գումարի, արժույթի, ցանցի կամ վճարման եղանակի համար առաջարկ կարող է չլինել։ Աղբյուրը կարող է նաև ժամանակավորապես չպատասխանել։ Փորձեք այլ գումար, հարթակներ կամ վճարման եղանակ։" },
    ],
    projectTitle: "Նախագծի մասին և հետադարձ կապ",
    project: "Pay3Flow-ը զարգանում է որպես բաց կոդով Flow3Pay նախագիծ GitHub-ում։ Այնտեղ կարող եք ուսումնասիրել կոդն ու հաշվարկները և հաղորդել սխալի մասին Issues բաժնում։ Նախագիծն ունի նաև Telegram համայնք։ Օգտագործման կանոնները նկարագրում են ծառայության հնարավորություններն ու սահմանափակումները։",
    github: "Կոդ և սխալների հաղորդումներ",
    telegram: "Telegram համայնք",
    terms: "Օգտագործման կանոններ",
  },
};

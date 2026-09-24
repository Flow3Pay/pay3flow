import { writable } from "svelte/store";

export type Locale = "en" | "ru" | "hy";

const STORAGE_KEY = "pay3flow-locale";
const supportedLocales: Locale[] = ["en", "ru", "hy"];

const messages: Record<Locale, Record<string, string>> = {
  en: {},
  ru: {
    "Switch language": "Переключить язык",
    "Switch theme": "Переключить тему",
    "Pay3Flow home": "Главная Pay3Flow",
    "Live routing infrastructure · Public market estimates": "Инфраструктура маршрутизации · Публичные оценки рынка",
    "Move money.": "Переводите деньги.",
    "Keep more.": "Сохраняйте больше.",
    "Stop spending hours searching for an exchange.": "Не тратьте часы на поиск обмена.",
    "Exchange mode": "Режим обмена",
    Bridge: "Маршрут",
    History: "История",
    "Refresh routes now": "Обновить маршруты",
    "Route refresh settings": "Настройки обновления маршрутов",
    "Refresh settings": "Настройки обновления",
    "Close route settings": "Закрыть настройки маршрута",
    "Auto-refresh": "Автообновление",
    "Keep market routes current": "Поддерживать маршруты актуальными",
    On: "Вкл.",
    Off: "Выкл.",
    "Search exchanges": "Искать на биржах",
    "Cryptocurrency intermediary": "Криптовалюта-посредник",
    "All available": "Все доступные",
    "Search also runs automatically 650ms after you change the amount, bank or intermediary.": "Поиск запускается автоматически через 650 мс после изменения суммы, банка или посредника.",
    Sell: "Продать",
    Buy: "Купить",
    "You send": "Вы отправляете",
    "Recipient gets": "Получатель получает",
    "digital wallet": "цифровой кошелёк",
    "bank transfer": "банковский перевод",
    "available via digital wallet": "доступно через цифровой кошелёк",
    "available via bank transfer": "доступно через банковский перевод",
    "Select bank": "Выберите банк",
    "Loading networks…": "Загрузка сетей…",
    Unavailable: "Недоступно",
    Network: "Сеть",
    "Swap sender and recipient": "Поменять отправителя и получателя",
    "Live estimate appears here": "Здесь появится актуальная оценка",
    "Public P2P sources only · no order placement": "Только публичные P2P-источники · без размещения ордера",
    "Auto-refresh is off": "Автообновление выключено",
    "Open swap instructions": "Открыть инструкции обмена",
    "Find routes": "Найти маршруты",
    "Finding routes": "Поиск маршрутов",
    "Enter an amount to begin": "Введите сумму, чтобы начать",
    "Choose where you pay from": "Выберите источник оплаты",
    "Choose where the recipient gets paid": "Выберите способ получения",
    "Found routes": "Найденные маршруты",
    "Awaiting your intent": "Ожидаем параметры обмена",
    "Best route": "Лучший маршрут",
    "Same output": "Та же сумма",
    "Showing top {count}": "Показаны лучшие: {count}",
    "No routes found": "Маршруты не найдены",
    "Preparing market scan": "Подготавливаем поиск по рынку",
    "Your routes will appear here": "Здесь появятся ваши маршруты",
    "No compatible live offers were found for this amount and payment method.": "Для этой суммы и способа оплаты не найдено подходящих предложений.",
    "Pay3Flow is ready to compare entry assets, venues and recipient payout options.": "Pay3Flow готов сравнить активы, площадки и варианты выплаты получателю.",
    "Enter an amount and we will assemble live cross-border paths in real time.": "Введите сумму — мы соберём актуальные трансграничные маршруты в реальном времени.",
    "Searching live routes": "Поиск актуальных маршрутов",
    "Route feedback": "Оценка маршрута",
    "Used {count} times": "Использован: {count} раз",
    "Network: {network}": "Сеть: {network}",
    "Like {name} route": "Нравится маршрут {name}",
    "Dislike {name} route": "Не нравится маршрут {name}",
    "Choose network": "Выберите сеть",
    "Close network picker": "Закрыть выбор сети",
    "Crypto networks": "Криптовалютные сети",
    "Available networks": "Доступные сети",
    "Close payment method picker": "Закрыть выбор способа оплаты",
    "Search banks, assets or networks…": "Поиск банков, активов или сетей…",
    "Search banks and payment methods": "Поиск банков и способов оплаты",
    "Payment methods": "Способы оплаты",
    "No payment methods found": "Способы оплаты не найдены",
    "Try a different bank name.": "Попробуйте другое название банка.",
    "Popular banks": "Популярные банки",
    "Digital assets": "Цифровые активы",
    "All payment methods": "Все способы оплаты",
    "Bank transfer": "Банковский перевод",
    "Digital wallet": "Цифровой кошелёк",
    "Selected route": "Выбранный маршрут",
    "How to complete this exchange": "Как выполнить этот обмен",
    "Complete each step in order. You stay in control—Pay3Flow never places an order or moves your funds.": "Выполняйте шаги по порядку. Вы контролируете процесс — Pay3Flow не размещает ордера и не перемещает ваши средства.",
    "Estimated output:": "Расчётный результат:",
    "Close instructions": "Закрыть инструкции",
    "Exchange steps": "Шаги обмена",
    Important: "Важно",
    "Rates, limits and offers can change. Confirm the provider or counterparty, payment details, and network before sending money. Pay3Flow never creates the order or moves funds.": "Курсы, лимиты и предложения могут измениться. Перед отправкой денег проверьте поставщика или контрагента, платёжные данные и сеть. Pay3Flow не создаёт ордера и не перемещает средства.",
    "Advertiser details unavailable": "Данные рекламодателя недоступны",
    "Direct exchange": "Прямой обмен",
    "User profile": "Профиль пользователя",
    "Find by nickname": "Найти по никнейму",
    "Exchange service": "Сервис обмена",
    Merchant: "Мерчант",
    Advertiser: "Рекламодатель",
    Settlement: "Расчёт",
    Payment: "Оплата",
    "confirm on provider": "уточните у поставщика",
    completion: "исполнено",
    "orders / 30d": "ордеров / 30 дн.",
    rate: "курс",
    "Open {venue} exchange": "Открыть обмен {venue}",
    "Open {venue} profile": "Открыть профиль {venue}",
    "Open {venue} P2P and find {nickname}": "Открыть P2P {venue} и найти {nickname}",
    "Match the nickname and ad ID {id} before opening an order.": "Перед открытием ордера сверьте никнейм и ID объявления {id}.",
    "Search {label}": "Искать на {label}",
    "Select {label}": "Выбрать {label}",
    "Refresh in {seconds} seconds": "Обновление через {seconds} сек.",
    "Updated {seconds}s ago": "Обновлено {seconds} сек. назад",
    "Conversion rate": "Курс конвертации",
    "Spot market": "Спотовый рынок",
    "Review the live price, trading fee, and expected amount before submitting the order.": "Перед отправкой ордера проверьте актуальную цену, торговую комиссию и ожидаемую сумму.",
    "Wait until the converted balance is available before continuing.": "Прежде чем продолжить, дождитесь доступности конвертированного баланса.",
    "Review the live price, trading fee, and final amount before submitting the order.": "Перед отправкой ордера проверьте актуальную цену, торговую комиссию и итоговую сумму.",
    "Confirm the destination asset and network before withdrawing it from the venue.": "Перед выводом с площадки проверьте актив назначения и сеть.",
    "Confirm the currencies, amount, live rate, fees, and limits before continuing.": "Перед продолжением проверьте валюты, сумму, актуальный курс, комиссии и лимиты.",
    "Confirm the converted balance is available before continuing to the next step.": "Перед переходом к следующему шагу убедитесь, что конвертированный баланс доступен.",
    "Match the advertiser nickname and ad ID before creating the order.": "Перед созданием ордера сверьте никнейм рекламодателя и ID объявления.",
    "Check the full address, any required memo or tag, and the withdrawal fee before confirming.": "Перед подтверждением проверьте полный адрес, обязательный memo или тег и комиссию за вывод.",
    "Confirm the payout reached your destination account before considering the exchange complete.": "Прежде чем считать обмен завершённым, убедитесь, что выплата поступила на ваш счёт.",
    "Release the asset only after you have independently confirmed receipt of the payment.": "Освобождайте актив только после самостоятельного подтверждения получения оплаты.",
    "Use only the payment details shown inside the order, then mark it paid after sending the transfer.": "Используйте только платёжные реквизиты из ордера, а после перевода отметьте его как оплаченный.",
    "Release the asset only after you have independently confirmed the payment in your bank or payment account.": "Освобождайте актив только после самостоятельного подтверждения оплаты в банке или платёжном аккаунте.",
  },
  hy: {
    "Switch language": "Փոխել լեզուն",
    "Switch theme": "Փոխել թեման",
    "Pay3Flow home": "Pay3Flow-ի գլխավոր էջ",
    "Live routing infrastructure · Public market estimates": "Ուղղորդման ենթակառուցվածք · Շուկայի հանրային գնահատականներ",
    "Move money.": "Փոխանցեք գումարը։",
    "Keep more.": "Պահպանեք ավելին։",
    "Stop spending hours searching for an exchange.": "Այլևս ժամեր մի ծախսեք փոխանակում փնտրելու վրա։",
    "Exchange mode": "Փոխանակման ռեժիմ",
    Bridge: "Ուղղորդում",
    History: "Պատմություն",
    "Refresh routes now": "Թարմացնել ուղղությունները",
    "Route refresh settings": "Ուղղությունների թարմացման կարգավորումներ",
    "Refresh settings": "Թարմացման կարգավորումներ",
    "Close route settings": "Փակել ուղղության կարգավորումները",
    "Auto-refresh": "Ավտոմատ թարմացում",
    "Keep market routes current": "Պահել շուկայի ուղղությունները արդիական",
    On: "Միացված",
    Off: "Անջատված",
    "Search exchanges": "Փնտրել բորսաներում",
    "Cryptocurrency intermediary": "Միջնորդ կրիպտոարժույթ",
    "All available": "Բոլոր հասանելիները",
    "Search also runs automatically 650ms after you change the amount, bank or intermediary.": "Որոնումն ավտոմատ կսկսվի 650 մվ անց՝ գումարը, բանկը կամ միջնորդը փոխելուց հետո։",
    Sell: "Վաճառել",
    Buy: "Գնել",
    "You send": "Դուք ուղարկում եք",
    "Recipient gets": "Ստացողը ստանում է",
    "digital wallet": "թվային դրամապանակով",
    "bank transfer": "բանկային փոխանցմամբ",
    "Select bank": "Ընտրեք բանկը",
    "Loading networks…": "Ցանցերը բեռնվում են…",
    Unavailable: "Անհասանելի է",
    Network: "Ցանց",
    "Swap sender and recipient": "Փոխել ուղարկողին և ստացողին",
    "Live estimate appears here": "Այստեղ կհայտնվի ընթացիկ գնահատականը",
    "Public P2P sources only · no order placement": "Միայն հանրային P2P աղբյուրներ · պատվեր չի տեղադրվում",
    "Auto-refresh is off": "Ավտոմատ թարմացումն անջատված է",
    "Open swap instructions": "Բացել փոխանակման հրահանգները",
    "Find routes": "Գտնել ուղղություններ",
    "Finding routes": "Ուղղությունների որոնում",
    "Enter an amount to begin": "Մուտքագրեք գումարը՝ սկսելու համար",
    "Choose where you pay from": "Ընտրեք վճարման աղբյուրը",
    "Choose where the recipient gets paid": "Ընտրեք ստացման եղանակը",
    "Found routes": "Գտնված ուղղություններ",
    "Awaiting your intent": "Սպասում ենք փոխանակման տվյալներին",
    "Best route": "Լավագույն ուղղություն",
    "Same output": "Նույն արդյունքը",
    "Showing top {count}": "Ցուցադրվում են լավագույն {count}-ը",
    "No routes found": "Ուղղություններ չեն գտնվել",
    "Preparing market scan": "Պատրաստում ենք շուկայի որոնումը",
    "Your routes will appear here": "Ձեր ուղղությունները կհայտնվեն այստեղ",
    "No compatible live offers were found for this amount and payment method.": "Այս գումարի և վճարման եղանակի համար հարմար առաջարկներ չեն գտնվել։",
    "Pay3Flow is ready to compare entry assets, venues and recipient payout options.": "Pay3Flow-ը պատրաստ է համեմատել մուտքային ակտիվները, հարթակները և ստացողի վճարման տարբերակները։",
    "Enter an amount and we will assemble live cross-border paths in real time.": "Մուտքագրեք գումարը, և մենք իրական ժամանակում կհավաքենք միջսահմանային ուղիները։",
    "Searching live routes": "Ընթացիկ ուղղությունների որոնում",
    "Route feedback": "Ուղղության գնահատական",
    "Used {count} times": "Օգտագործվել է {count} անգամ",
    "Network: {network}": "Ցանց՝ {network}",
    "Like {name} route": "Հավանել {name}-ի ուղղությունը",
    "Dislike {name} route": "Չհավանել {name}-ի ուղղությունը",
    "Choose network": "Ընտրեք ցանցը",
    "Close network picker": "Փակել ցանցի ընտրությունը",
    "Crypto networks": "Կրիպտո ցանցեր",
    "Available networks": "Հասանելի ցանցեր",
    "Close payment method picker": "Փակել վճարման եղանակի ընտրությունը",
    "Search banks, assets or networks…": "Փնտրել բանկեր, ակտիվներ կամ ցանցեր…",
    "Search banks and payment methods": "Փնտրել բանկեր և վճարման եղանակներ",
    "Payment methods": "Վճարման եղանակներ",
    "No payment methods found": "Վճարման եղանակներ չեն գտնվել",
    "Try a different bank name.": "Փորձեք բանկի այլ անվանում։",
    "Popular banks": "Հանրաճանաչ բանկեր",
    "Digital assets": "Թվային ակտիվներ",
    "All payment methods": "Վճարման բոլոր եղանակները",
    "Bank transfer": "Բանկային փոխանցում",
    "Digital wallet": "Թվային դրամապանակ",
    "Selected route": "Ընտրված ուղղություն",
    "How to complete this exchange": "Ինչպես կատարել այս փոխանակումը",
    "Complete each step in order. You stay in control—Pay3Flow never places an order or moves your funds.": "Կատարեք քայլերը հերթականությամբ։ Դուք եք վերահսկում գործընթացը․ Pay3Flow-ը պատվեր չի տեղադրում և միջոցներ չի տեղափոխում։",
    "Estimated output:": "Մոտավոր արդյունք՝",
    "Close instructions": "Փակել հրահանգները",
    "Exchange steps": "Փոխանակման քայլերը",
    Important: "Կարևոր է",
    "Rates, limits and offers can change. Confirm the provider or counterparty, payment details, and network before sending money. Pay3Flow never creates the order or moves funds.": "Փոխարժեքները, սահմանաչափերը և առաջարկները կարող են փոխվել։ Գումար ուղարկելուց առաջ ստուգեք մատակարարին կամ հակակողմին, վճարման տվյալները և ցանցը։ Pay3Flow-ը պատվեր չի ստեղծում և միջոցներ չի տեղափոխում։",
    "Advertiser details unavailable": "Գովազդատուի տվյալները հասանելի չեն",
    "Direct exchange": "Ուղղակի փոխանակում",
    "User profile": "Օգտատիրոջ պրոֆիլ",
    "Find by nickname": "Գտնել մականունով",
    "Exchange service": "Փոխանակման ծառայություն",
    Merchant: "Մերչանտ",
    Advertiser: "Գովազդատու",
    Settlement: "Հաշվարկ",
    Payment: "Վճարում",
    "confirm on provider": "ճշտեք մատակարարից",
    completion: "կատարում",
    "orders / 30d": "պատվեր / 30 օրում",
    rate: "փոխարժեք",
    "Open {venue} exchange": "Բացել {venue}-ի փոխանակումը",
    "Open {venue} profile": "Բացել {venue}-ի պրոֆիլը",
    "Open {venue} P2P and find {nickname}": "Բացել {venue}-ի P2P-ն և գտնել {nickname}-ին",
    "Match the nickname and ad ID {id} before opening an order.": "Պատվեր բացելուց առաջ համեմատեք մականունը և հայտարարության ID-ն՝ {id}։",
    "Search {label}": "Փնտրել {label}-ում",
    "Select {label}": "Ընտրել {label}-ը",
    "Refresh in {seconds} seconds": "Թարմացում՝ {seconds} վրկ-ից",
    "Updated {seconds}s ago": "Թարմացվել է {seconds} վրկ. առաջ",
    "Conversion rate": "Փոխարկման փոխարժեք",
    "Spot market": "Սփոթ շուկա",
    "Review the live price, trading fee, and expected amount before submitting the order.": "Պատվերը ուղարկելուց առաջ ստուգեք ընթացիկ գինը, առևտրային միջնորդավճարը և ակնկալվող գումարը։",
    "Wait until the converted balance is available before continuing.": "Շարունակելուց առաջ սպասեք, մինչև փոխարկված մնացորդը հասանելի լինի։",
    "Review the live price, trading fee, and final amount before submitting the order.": "Պատվերը ուղարկելուց առաջ ստուգեք ընթացիկ գինը, առևտրային միջնորդավճարը և վերջնական գումարը։",
    "Confirm the destination asset and network before withdrawing it from the venue.": "Հարթակից դուրս բերելուց առաջ հաստատեք նպատակային ակտիվը և ցանցը։",
    "Confirm the currencies, amount, live rate, fees, and limits before continuing.": "Շարունակելուց առաջ հաստատեք արժույթները, գումարը, ընթացիկ փոխարժեքը, միջնորդավճարներն ու սահմանաչափերը։",
    "Confirm the converted balance is available before continuing to the next step.": "Հաջորդ քայլին անցնելուց առաջ համոզվեք, որ փոխարկված մնացորդը հասանելի է։",
    "Match the advertiser nickname and ad ID before creating the order.": "Պատվեր ստեղծելուց առաջ համեմատեք գովազդատուի մականունը և հայտարարության ID-ն։",
    "Check the full address, any required memo or tag, and the withdrawal fee before confirming.": "Հաստատելուց առաջ ստուգեք ամբողջական հասցեն, անհրաժեշտ memo-ն կամ թեգը և դուրսբերման միջնորդավճարը։",
    "Confirm the payout reached your destination account before considering the exchange complete.": "Փոխանակումն ավարտված համարելուց առաջ համոզվեք, որ վճարումը հասել է ձեր հաշվին։",
    "Release the asset only after you have independently confirmed receipt of the payment.": "Ակտիվը բաց թողեք միայն վճարման ստացումը ինքնուրույն հաստատելուց հետո։",
    "Use only the payment details shown inside the order, then mark it paid after sending the transfer.": "Օգտագործեք միայն պատվերի մեջ նշված վճարման տվյալները և փոխանցումից հետո նշեք այն որպես վճարված։",
    "Release the asset only after you have independently confirmed the payment in your bank or payment account.": "Ակտիվը բաց թողեք միայն բանկում կամ վճարային հաշվին վճարումը ինքնուրույն հաստատելուց հետո։",
  },
};

function readLocale(): Locale {
  if (typeof window === "undefined") return "en";
  const stored = window.localStorage.getItem(STORAGE_KEY) as Locale | null;
  return stored && supportedLocales.includes(stored) ? stored : "en";
}

export const locale = writable<Locale>(readLocale());

export function setLocale(next: Locale) {
  locale.set(next);
  if (typeof document !== "undefined") document.documentElement.lang = next;
  if (typeof window !== "undefined") window.localStorage.setItem(STORAGE_KEY, next);
}

export function cycleLocale(current: Locale): Locale {
  return supportedLocales[(supportedLocales.indexOf(current) + 1) % supportedLocales.length];
}

export function t(key: string, params: Record<string, string | number> = {}, language: Locale = "en") {
  let value = messages[language][key] ?? key;
  for (const [name, replacement] of Object.entries(params)) value = value.replaceAll(`{${name}}`, String(replacement));
  return value;
}

export const localeLabel = (language: Locale) => language === "ru" ? "RU" : language === "hy" ? "ՀԱՅ" : "EN";

/**
 * Translates legacy static copy in components that have not yet been migrated
 * to `t(...)`. Original text is kept per node so changing languages is safe.
 */
export function localize(node: HTMLElement) {
  const originalText = new WeakMap<Text, string>();
  const originalAttributes = new WeakMap<Element, Map<string, string>>();
  const attributes = ["aria-label", "title", "placeholder"];

  function translate(value: string, language: Locale) {
    const trimmed = value.trim();
    let translated = t(trimmed, {}, language);
    if (translated === trimmed) {
      let match = trimmed.match(/^(.*?) available via (digital wallet|bank transfer)$/);
      if (match) translated = `${match[1]} ${t(`available via ${match[2]}`, {}, language)}`;
      match = trimmed.match(/^Estimated (.+)$/);
      if (match) translated = `${language === "ru" ? "Расчётный" : language === "hy" ? "Մոտավոր" : "Estimated"} ${match[1]}`;
      match = trimmed.match(/^Updated (\d+)s ago$/);
      if (match) translated = t("Updated {seconds}s ago", { seconds: match[1] }, language);
      match = trimmed.match(/^Refresh in (\d+) seconds$/);
      if (match) translated = t("Refresh in {seconds} seconds", { seconds: match[1] }, language);
      match = trimmed.match(/^Showing top (\d+)$/);
      if (match) translated = t("Showing top {count}", { count: match[1] }, language);
      match = trimmed.match(/^(\d+) (route|routes) found$/);
      if (match) translated = `${match[1]} ${language === "ru" ? "маршрут" : language === "hy" ? "ուղղություն" : match[2]} ${language === "ru" ? "найдено" : language === "hy" ? "գտնվել է" : "found"}`;
      match = trimmed.match(/^Send ([^ ]+) → ([^ ]+)$/);
      if (match) translated = `${language === "ru" ? "Отправить" : language === "hy" ? "Ուղարկել" : "Send"} ${match[1]} → ${match[2]}`;
      match = trimmed.match(/^Search (.+)$/);
      if (match) translated = t("Search {label}", { label: match[1] }, language);
      match = trimmed.match(/^Select (.+)$/);
      if (match) translated = t("Select {label}", { label: match[1] }, language);
    }
    return translated === trimmed ? value : value.replace(trimmed, translated);
  }

  function translateTree(root: Node, language: Locale) {
    const walker = document.createTreeWalker(root, NodeFilter.SHOW_TEXT);
    const textNodes: Text[] = [];
    let current: Node | null;
    while ((current = walker.nextNode())) textNodes.push(current as Text);
    for (const text of textNodes) {
      if (!originalText.has(text)) originalText.set(text, text.nodeValue ?? "");
      const source = originalText.get(text) ?? "";
      if (source.trim()) text.nodeValue = translate(source, language);
    }
    const elements = root instanceof Element ? [root, ...Array.from(root.querySelectorAll("*"))] : Array.from((root as ParentNode).querySelectorAll?.("*") ?? []);
    for (const element of elements) {
      if (!originalAttributes.has(element)) originalAttributes.set(element, new Map());
      const saved = originalAttributes.get(element)!;
      for (const attribute of attributes) {
        const value = element.getAttribute(attribute);
        if (value === null) continue;
        if (!saved.has(attribute)) saved.set(attribute, value);
        const source = saved.get(attribute)!;
        element.setAttribute(attribute, translate(source, language));
      }
    }
  }

  const unsubscribe = locale.subscribe((language) => translateTree(node, language));
  const observer = new MutationObserver((records) => {
    for (const record of records) for (const added of record.addedNodes) translateTree(added, readLocale());
  });
  observer.observe(node, { childList: true, subtree: true });
  return { destroy() { unsubscribe(); observer.disconnect(); } };
}

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
    cash: "наличные",
    "available via digital wallet": "доступно через цифровой кошелёк",
    "available via bank transfer": "доступно через банковский перевод",
    "available via cash": "доступно наличными",
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
    "Like this route": "Нравится этот маршрут",
    "Dislike this route": "Не нравится этот маршрут",
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
    "Popular methods": "Популярные способы",
    "Digital assets": "Цифровые активы",
    "All payment methods": "Все способы оплаты",
    "Bank transfer": "Банковский перевод",
    "Cash settlement": "Расчёт наличными",
    "Cash settlement · USD": "Наличные · USD",
    "USD · Cash settlement": "USD · Наличные",
    "Cash USD": "Наличные USD",
    "US dollar": "Доллар США",
    International: "Международный",
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
    "Convert {from} to {to}": "Конвертировать {from} в {to}",
    "Use the {pair} spot market on {venue}. This is an exchange order book, so there is no P2P advertiser to contact.": "Используйте спотовый рынок {pair} на {venue}. Это биржевой стакан, поэтому связываться с P2P-рекламодателем не нужно.",
    "Confirm the pair converts {from} into {to}.": "Убедитесь, что пара конвертирует {from} в {to}.",
    "Open {pair} and confirm it converts {from} into {to}.": "Откройте {pair} и убедитесь, что он конвертирует {from} в {to}.",
    "Conversion rate {rate}": "Курс конвертации {rate}",
    "Open {pair} on {venue}": "Открыть {pair} на {venue}",
    "Complete the second conversion on {venue} only after the first trade has settled into your available balance.": "Выполняйте вторую конвертацию на {venue} только после зачисления первой сделки на доступный баланс.",
    "Open {role}'s profile on {venue} and create the first P2P order.": "Откройте профиль {role} на {venue} и создайте первый P2P-ордер.",
    "Open the seller's profile on {venue}, create the P2P order, and pay with the selected payment method.": "Откройте профиль продавца на {venue}, создайте P2P-ордер и оплатите выбранным способом.",
    "Open the buyer's profile on {venue} and create the sell order using the selected recipient payment method.": "Откройте профиль покупателя на {venue} и создайте ордер на продажу выбранным способом получения.",
    "Open the direct exchange on {venue}, review the live quote, and complete the conversion in the provider flow.": "Откройте прямой обмен на {venue}, проверьте актуальную котировку и завершите конвертацию в интерфейсе поставщика.",
    "Complete any login or verification required by {venue} and follow its payment instructions.": "Пройдите необходимый вход или проверку на {venue} и следуйте его платёжным инструкциям.",
    "Complete any login or verification required by {venue} and follow its transfer instructions.": "Пройдите необходимый вход или проверку на {venue} и следуйте его инструкциям по переводу.",
    "Confirm the live rate, order limits, and payment method on {venue}.": "Проверьте актуальный курс, лимиты ордера и способ оплаты на {venue}.",
    "Transfer {asset} to {venue}": "Перевести {asset} на {venue}",
    "Move the purchased asset from {from} to your deposit address on {to} before opening the next P2P order.": "Переведите купленный актив с {from} на депозитный адрес {to} перед открытием следующего P2P-ордера.",
    "Copy the deposit address from {venue} and select the exact {network} network on both venues.": "Скопируйте депозитный адрес в {venue} и выберите точную сеть {network} на обеих площадках.",
    "Confirm that both venues support the same asset and network, then copy the deposit address from {venue}.": "Убедитесь, что обе площадки поддерживают один актив и сеть, затем скопируйте депозитный адрес в {venue}.",
    "Wait for {venue} to credit the deposit before continuing.": "Прежде чем продолжить, дождитесь зачисления депозита на {venue}.",
    "Buy {asset} for {amount}": "Купить {asset} за {amount}",
    "Sell {asset} for {amount}": "Продать {asset} за {amount}",
    "Buy {asset} with {bridge}": "Купить {asset} за {bridge}",
    "Confirm the {asset} balance and network before withdrawing.": "Перед выводом проверьте баланс {asset} и сеть.",
    "Spot-market estimate only: trading fees, slippage and execution are not guaranteed.": "Только оценка спотового рынка: торговые комиссии, проскальзывание и исполнение не гарантированы.",
    "Deposit and withdrawal network availability and fees are not verified by the selected venue.": "Выбранная площадка не проверяет доступность сетей и комиссии за пополнение и вывод.",
    "Bank fees:": "Комиссии банков:",
    "{bank}: no bank fee": "{bank}: комиссии банка нет",
    "{bank}: {percent}% bank fee": "{bank}: комиссия банка {percent}%",
    "For RUB, use SBP from {bank} using the exact recipient details shown in the order.": "Для RUB используйте СБП из банка {bank}, указав точные реквизиты получателя из ордера.",
    "For RUB, use SBP from {bank} only if the advertiser lists it; otherwise use the payment method shown in the order.": "Для RUB используйте СБП из банка {bank} только если оно указано в объявлении. Иначе используйте способ оплаты из ордера.",
    "For RUB payout to {bank}, confirm the SBP transfer has arrived before releasing the crypto.": "При выплате RUB на {bank} убедитесь, что перевод через СБП поступил, и только потом передавайте криптовалюту.",
    "For RUB payout to {bank}, use SBP only if the order supports it and confirm the money has arrived before releasing the crypto.": "При выплате RUB на {bank} используйте СБП только если ордер его поддерживает. Перед передачей криптовалюты убедитесь, что деньги поступили.",
    "Open {venue}": "Открыть {venue}",
    "Direct exchange on {venue}": "Прямой обмен на {venue}",
    "on {venue}": "на {venue}",
    Buyer: "Покупатель",
    Seller: "Продавец",
    "asset network": "сеть актива",
    "recipient payment method": "способ получения денег",
    "Close instructions by dragging down": "Закрыть инструкции, потянув вниз",
    "Route through {venue}": "Маршрут через {venue}",
    "This is a current estimate only. Pay3Flow does not send money or make the exchange for you.": "Это только текущий расчёт. Pay3Flow не отправляет деньги и не выполняет обмен за вас.",
    "Check which asset and network you send, and which asset and network you receive.": "Проверьте, какой актив и по какой сети вы отправляете и какой актив и сеть получаете.",
    "Check the amount you will receive, the provider fee, how long the quote is valid, and whether a memo or tag is required.": "Проверьте сумму к получению, комиссию поставщика, срок действия расчёта и необходимость memo или тега.",
    "Never send money after the quote expires. Get a new quote first.": "Никогда не отправляйте деньги после окончания срока расчёта. Сначала получите новый расчёт.",
    "This is a normal exchange on {venue}. There is no separate person to message.": "Это обычный обмен на площадке {venue}. Отдельному человеку писать не нужно.",
    "First check that the pair changes {from} into {to}.": "Сначала проверьте, что эта пара меняет {from} на {to}.",
    "Check the current price, fee, and amount you should receive before pressing the exchange button.": "Перед нажатием кнопки обмена проверьте текущую цену, комиссию и сумму к получению.",
    "Wait until the new balance appears before doing the next step.": "Перед следующим шагом дождитесь появления новой суммы на балансе.",
    "Do the second exchange on {venue} only after the first balance is available.": "Делайте второй обмен на {venue} только после появления суммы от первого обмена на балансе.",
    "Open {pair} and check that it changes {from} into {to}.": "Откройте {pair} и проверьте, что он меняет {from} на {to}.",
    "Before withdrawing, check the receiving asset and the network one more time.": "Перед выводом ещё раз проверьте актив получателя и сеть.",
    "Transfer {from} to {to} via {venue}": "Перевести {from} в {to} через {venue}",
    "Open the direct exchange on {venue}, check the final amount, and follow the provider's instructions.": "Откройте прямой обмен на {venue}, проверьте итоговую сумму и следуйте инструкциям поставщика.",
    "Check the currencies, amount, current rate, fee, and limits before continuing.": "Перед продолжением проверьте валюты, сумму, текущий курс, комиссию и лимиты.",
    "Sign in or complete verification on {venue}, if it asks you to, then follow the payment instructions shown there.": "Если {venue} попросит, войдите в аккаунт или пройдите проверку, затем следуйте показанным там платёжным инструкциям.",
    "After the exchange, check that the new balance is available before continuing.": "После обмена проверьте, что новая сумма доступна на балансе, и только потом продолжайте.",
    "Open the buyer's profile on {venue} and create the first P2P order.": "Откройте профиль покупателя на {venue} и создайте первый P2P-ордер.",
    "Open the seller's profile on {venue}, create the P2P order, and pay using the selected method.": "Откройте профиль продавца на {venue}, создайте P2P-ордер и оплатите выбранным способом.",
    "Before creating the order, compare the nickname and advertisement ID.": "Перед созданием ордера сверьте никнейм и ID объявления.",
    "Check the current rate, order limits, and payment method on {venue}.": "Проверьте на {venue} текущий курс, лимиты ордера и способ оплаты.",
    "This bank fee is only an estimate. Check the final bank fee before sending.": "Комиссия банка указана примерно. Перед отправкой проверьте её точный размер.",
    "Use only the payment details shown inside the order. After sending, mark the order as paid.": "Используйте только реквизиты из открытого ордера. После отправки денег отметьте ордер как оплаченный.",
    "Release the asset only after you personally see that the payment has arrived.": "Передавайте актив только после того, как лично убедитесь, что деньги поступили.",
    "Swap {from} for {to} via {venue}": "Обменять {from} на {to} через {venue}",
    "After you get {from}, send it to {venue} and exchange it for {to}.": "После получения {from} отправьте его на {venue} и обменяйте на {to}.",
    "Send {from} to {venue} first, then exchange it for {to}.": "Сначала отправьте {from} на {venue}, затем обменяйте его на {to}.",
    "Before sending, check the asset, the receiving asset, and the exact network.": "Перед отправкой проверьте актив, актив получателя и точную сеть.",
    "Check the current rate, provider fee, quote expiry, and any address, memo, or tag requirement.": "Проверьте текущий курс, комиссию поставщика, срок действия расчёта и требования к адресу, memo или тегу.",
    "Wait until the new balance appears before considering this step finished.": "Считайте этот шаг законченным только после появления новой суммы на балансе.",
    "Send the purchased asset from {from} to the deposit address on {to} before opening the next order.": "Перед следующим ордером отправьте купленный актив с {from} на депозитный адрес {to}.",
    "Copy the deposit address from {venue}. Choose the exact {network} network on both platforms.": "Скопируйте депозитный адрес в {venue}. На обеих площадках выберите одну и ту же сеть {network}.",
    "First check that both platforms support the same asset and network. Then copy the deposit address from {venue}.": "Сначала проверьте, что обе площадки поддерживают один актив и одну сеть. Затем скопируйте депозитный адрес в {venue}.",
    "Check the complete address, memo or tag if required, and the withdrawal fee before confirming.": "Перед подтверждением проверьте полный адрес, обязательный memo или тег и комиссию за вывод.",
    "Wait until {venue} shows the deposit as received before continuing.": "Перед продолжением дождитесь, пока {venue} покажет зачисление депозита.",
    "Open the seller's profile on {venue} and create the order for the asset you want to receive.": "Откройте профиль продавца на {venue} и создайте ордер на актив, который хотите получить.",
    "Open the buyer's profile on {venue} and create a sell order using the selected payment method.": "Откройте профиль покупателя на {venue} и создайте ордер на продажу с выбранным способом получения.",
    "Check the current rate, order limits, {thing}, and expected amount.": "Проверьте текущий курс, лимиты ордера, {thing} и ожидаемую сумму.",
    "This bank fee is only an estimate. Check the final fee before accepting the payout.": "Комиссия банка указана примерно. Перед получением выплаты проверьте её точный размер.",
    "After the sale, check that the money has arrived in your account before considering the exchange finished.": "После продажи проверьте, что деньги поступили на ваш счёт, и только тогда считайте обмен завершённым.",
    "Release the asset only after you personally see the payment in your bank or payment account.": "Передавайте актив только после того, как лично увидите оплату в банке или платёжном аккаунте.",
    "Important: rates, limits, and offers can change. Before sending money, check the provider or person, payment details, and network. Pay3Flow does not create orders or move money.": "Важно: курсы, лимиты и предложения могут измениться. Перед отправкой денег проверьте поставщика или человека, реквизиты и сеть. Pay3Flow не создаёт ордера и не переводит деньги.",
    "Rates, limits, and offers can change. Check the provider, payment details, and network before sending money. Pay3Flow does not create orders or move money.": "Курсы, лимиты и предложения могут измениться. Перед отправкой денег проверьте поставщика, реквизиты и сеть. Pay3Flow не создаёт ордера и не переводит деньги.",
    "Live dry quote from {provider}; execution and wallet compatibility are not verified.": "Текущий предварительный расчёт от {provider}; выполнение и совместимость кошельков не проверены.",
    "Quoted exchanger: {description}.": "Поставщик по расчёту: {description}.",
    "Cross-network transfer requires the provider's deposit and withdrawal flow; confirm addresses, memos, network fees, and finality before sending.": "Перевод между сетями требует пополнения и вывода у поставщика. Перед отправкой проверьте адреса, memo, комиссии сети и окончательное зачисление.",
    "Search estimate only: platform fees, account eligibility and execution are not verified.": "Это только предварительный расчёт: комиссии площадки, возможность использования аккаунта и выполнение не проверены.",
    "Cross-venue route requires an asset transfer; network fee and compatible network are not included.": "Маршрут через разные площадки требует перевода актива; комиссия сети и совместимость сети не включены в расчёт.",
    "The selected sender bank could not be verified because the venue returned an opaque payment-method ID.": "Выбранный банк отправителя не удалось проверить: площадка вернула непонятный ID способа оплаты.",
    "The selected recipient bank could not be verified because the venue returned an opaque payment-method ID.": "Выбранный банк получателя не удалось проверить: площадка вернула непонятный ID способа оплаты.",
    "At least one selected venue does not expose a public deep-link for this advertisement. Verify the advertiser ID and terms on the venue before sending money.": "Одна из выбранных площадок не даёт прямую ссылку на это объявление. Перед отправкой денег проверьте на площадке ID рекламодателя и условия.",
    "Live fiat entry/exit offers plus a live dry cross-network quote; platform limits and execution are not verified.": "Актуальные предложения на вход и выход плюс предварительный расчёт перевода между сетями; лимиты площадки и выполнение не проверены.",
    "Confirm the source and destination networks, provider deposit address, memo/tag, network fee, and finality before sending.": "Перед отправкой проверьте исходную и конечную сети, депозитный адрес поставщика, memo/tag, комиссию сети и окончательное зачисление.",
    "Indicative direct-transfer quote; confirm the live rate, account eligibility, transfer limits, and recipient details with the provider before sending.": "Предварительный расчёт прямого перевода. Перед отправкой уточните у поставщика текущий курс, возможность использования аккаунта, лимиты перевода и реквизиты получателя.",
    "The selected sender payment method could not be verified by the venue.": "Площадка не смогла проверить выбранный способ оплаты отправителя.",
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
    cash: "կանխիկ",
    "available via digital wallet": "հասանելի է թվային դրամապանակով",
    "available via bank transfer": "հասանելի է բանկային փոխանցմամբ",
    "available via cash": "հասանելի է կանխիկով",
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
    "Like this route": "Հավանել այս ուղղությունը",
    "Dislike this route": "Չհավանել այս ուղղությունը",
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
    "Popular methods": "Հանրաճանաչ եղանակներ",
    "Digital assets": "Թվային ակտիվներ",
    "All payment methods": "Վճարման բոլոր եղանակները",
    "Bank transfer": "Բանկային փոխանցում",
    "Cash settlement": "Կանխիկ հաշվարկ",
    "Cash settlement · USD": "Կանխիկ · USD",
    "USD · Cash settlement": "USD · Կանխիկ",
    "Cash USD": "Կանխիկ USD",
    "US dollar": "ԱՄՆ դոլար",
    International: "Միջազգային",
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
    "Use the {pair} spot market on {venue}. This is an exchange order book, so there is no P2P advertiser to contact.": "Օգտագործեք {venue}-ի {pair} սփոթ շուկան։ Սա բորսայի պատվերների գիրք է, ուստի P2P գովազդատուի հետ կապվել պետք չէ։",
    "Confirm the pair converts {from} into {to}.": "Հաստատեք, որ զույգը փոխարկում է {from}-ը {to}-ի։",
    "Open {pair} and confirm it converts {from} into {to}.": "Բացեք {pair}-ը և հաստատեք, որ այն փոխարկում է {from}-ը {to}-ի։",
    "Conversion rate {rate}": "Փոխարկման փոխարժեք՝ {rate}",
    "Open {pair} on {venue}": "Բացել {pair}-ը {venue}-ում",
    "Complete the second conversion on {venue} only after the first trade has settled into your available balance.": "Երկրորդ փոխարկումը {venue}-ում կատարեք միայն առաջին գործարքի հասանելի մնացորդում մուտքագրվելուց հետո։",
    "Open {role}'s profile on {venue} and create the first P2P order.": "Բացեք {role}-ի պրոֆիլը {venue}-ում և ստեղծեք առաջին P2P պատվերը։",
    "Open the seller's profile on {venue}, create the P2P order, and pay with the selected payment method.": "Բացեք վաճառողի պրոֆիլը {venue}-ում, ստեղծեք P2P պատվերը և վճարեք ընտրված եղանակով։",
    "Open the buyer's profile on {venue} and create the sell order using the selected recipient payment method.": "Բացեք գնորդի պրոֆիլը {venue}-ում և ստեղծեք վաճառքի պատվերը՝ օգտագործելով ստացման ընտրված եղանակը։",
    "Open the direct exchange on {venue}, review the live quote, and complete the conversion in the provider flow.": "Բացեք ուղղակի փոխանակումը {venue}-ում, ստուգեք ընթացիկ գնանշումը և ավարտեք փոխարկումը մատակարարի հոսքում։",
    "Complete any login or verification required by {venue} and follow its payment instructions.": "Կատարեք {venue}-ի պահանջած մուտքը կամ ստուգումը և հետևեք վճարման հրահանգներին։",
    "Complete any login or verification required by {venue} and follow its transfer instructions.": "Կատարեք {venue}-ի պահանջած մուտքը կամ ստուգումը և հետևեք փոխանցման հրահանգներին։",
    "Confirm the live rate, order limits, and payment method on {venue}.": "Ստուգեք ընթացիկ փոխարժեքը, պատվերի սահմանաչափերը և վճարման եղանակը {venue}-ում։",
    "Move the purchased asset from {from} to your deposit address on {to} before opening the next P2P order.": "Հաջորդ P2P պատվերը բացելուց առաջ գնված ակտիվը {from}-ից փոխանցեք {to}-ի ավանդի հասցեին։",
    "Copy the deposit address from {venue} and select the exact {network} network on both venues.": "Պատճենեք ավանդի հասցեն {venue}-ից և երկու հարթակներում ընտրեք ճիշտ {network} ցանցը։",
    "Confirm that both venues support the same asset and network, then copy the deposit address from {venue}.": "Հաստատեք, որ երկու հարթակներն էլ աջակցում են նույն ակտիվին և ցանցին, ապա պատճենեք ավանդի հասցեն {venue}-ից։",
    "Wait for {venue} to credit the deposit before continuing.": "Շարունակելուց առաջ սպասեք, մինչև {venue}-ն մուտքագրի ավանդը։",
    "Buy {asset} for {amount}": "Գնել {asset}-ը {amount}-ով",
    "Sell {asset} for {amount}": "Վաճառել {asset}-ը {amount}-ով",
    "Buy {asset} with {bridge}": "Գնել {asset}-ը {bridge}-ով",
    "Confirm the {asset} balance and network before withdrawing.": "Դուրս բերելուց առաջ ստուգեք {asset}-ի մնացորդը և ցանցը։",
    "Spot-market estimate only: trading fees, slippage and execution are not guaranteed.": "Միայն սփոթ շուկայի գնահատական է․ առևտրային միջնորդավճարները, սայթաքումը և կատարումը երաշխավորված չեն։",
    "Deposit and withdrawal network availability and fees are not verified by the selected venue.": "Ընտրված հարթակը չի ստուգում ցանցերի հասանելիությունն ու մուտքագրման և դուրսբերման միջնորդավճարները։",
    "Bank fees:": "Բանկային միջնորդավճարներ․",
    "{bank}: no bank fee": "{bank}․ բանկային միջնորդավճար չկա",
    "{bank}: {percent}% bank fee": "{bank}․ բանկային միջնորդավճար՝ {percent}%",
    "For RUB, use SBP from {bank} using the exact recipient details shown in the order.": "RUB-ի համար օգտագործեք {bank}-ի ՍԲՊ-ն՝ պատվերում նշված ստացողի ճշգրիտ տվյալներով։",
    "For RUB, use SBP from {bank} only if the advertiser lists it; otherwise use the payment method shown in the order.": "RUB-ի համար օգտագործեք {bank}-ի ՍԲՊ-ն միայն այն դեպքում, եթե այն նշված է հայտարարության մեջ․ հակառակ դեպքում օգտագործեք պատվերում նշված վճարման եղանակը։",
    "For RUB payout to {bank}, confirm the SBP transfer has arrived before releasing the crypto.": "RUB-ը {bank} ստանալիս համոզվեք, որ ՍԲՊ փոխանցումը հասել է, և միայն հետո փոխանցեք կրիպտոն։",
    "For RUB payout to {bank}, use SBP only if the order supports it and confirm the money has arrived before releasing the crypto.": "RUB-ը {bank} ստանալիս օգտագործեք ՍԲՊ միայն պատվերի աջակցման դեպքում և կրիպտոն փոխանցելուց առաջ համոզվեք, որ գումարը հասել է։",
    "Open {venue}": "Բացել {venue}",
    "Direct exchange on {venue}": "Ուղղակի փոխանակում {venue}-ում",
    "on {venue}": "{venue}-ում",
    Buyer: "Գնորդ",
    Seller: "Վաճառող",
    "asset network": "ակտիվի ցանցը",
    "recipient payment method": "գումարի ստացման եղանակը",
    "Close instructions by dragging down": "Փակել հրահանգները՝ ներքև քաշելով",
    "Route through {venue}": "Ուղղություն՝ {venue}-ով",
    "This is a current estimate only. Pay3Flow does not send money or make the exchange for you.": "Սա միայն ընթացիկ հաշվարկ է։ Pay3Flow-ը ձեր փոխարեն գումար չի ուղարկում և փոխանակում չի կատարում։",
    "Check which asset and network you send, and which asset and network you receive.": "Ստուգեք, թե որ ակտիվն ու ցանցն եք ուղարկում և որ ակտիվն ու ցանցն եք ստանում։",
    "Check the amount you will receive, the provider fee, how long the quote is valid, and whether a memo or tag is required.": "Ստուգեք ստացվող գումարը, մատակարարի միջնորդավճարը, հաշվարկի վավերականության ժամկետը և memo-ի կամ tag-ի անհրաժեշտությունը։",
    "Never send money after the quote expires. Get a new quote first.": "Երբեք գումար մի ուղարկեք հաշվարկի ժամկետի ավարտից հետո։ Նախ ստացեք նոր հաշվարկ։",
    "Convert {from} to {to}": "Փոխարկել {from}-ը {to}-ի",
    "This is a normal exchange on {venue}. There is no separate person to message.": "Սա սովորական փոխանակում է {venue}-ում։ Առանձին անձի գրել պետք չէ։",
    "First check that the pair changes {from} into {to}.": "Նախ ստուգեք, որ այս զույգը {from}-ը փոխում է {to}-ի։",
    "Check the current price, fee, and amount you should receive before pressing the exchange button.": "Փոխանակման կոճակը սեղմելուց առաջ ստուգեք ընթացիկ գինը, միջնորդավճարը և ստացվող գումարը։",
    "Wait until the new balance appears before doing the next step.": "Հաջորդ քայլին անցնելուց առաջ սպասեք նոր մնացորդի հայտնվելուն։",
    "Do the second exchange on {venue} only after the first balance is available.": "Երկրորդ փոխանակումը {venue}-ում կատարեք միայն առաջին մնացորդի հասանելի դառնալուց հետո։",
    "Open {pair} and check that it changes {from} into {to}.": "Բացեք {pair}-ը և ստուգեք, որ այն {from}-ը փոխում է {to}-ի։",
    "Before withdrawing, check the receiving asset and the network one more time.": "Դուրս բերելուց առաջ ևս մեկ անգամ ստուգեք ստացողի ակտիվն ու ցանցը։",
    "Transfer {from} to {to} via {venue}": "Փոխանցել {from}-ը {to}-ին {venue}-ով",
    "Open the direct exchange on {venue}, check the final amount, and follow the provider's instructions.": "Բացեք ուղղակի փոխանակումը {venue}-ում, ստուգեք վերջնական գումարը և հետևեք մատակարարի հրահանգներին։",
    "Check the currencies, amount, current rate, fee, and limits before continuing.": "Շարունակելուց առաջ ստուգեք արժույթները, գումարը, ընթացիկ փոխարժեքը, միջնորդավճարը և սահմանաչափերը։",
    "Sign in or complete verification on {venue}, if it asks you to, then follow the payment instructions shown there.": "Եթե {venue}-ը պահանջի, մուտք գործեք կամ անցեք ստուգում, ապա հետևեք այնտեղ ցուցադրված վճարման հրահանգներին։",
    "After the exchange, check that the new balance is available before continuing.": "Փոխանակումից հետո շարունակելուց առաջ ստուգեք, որ նոր մնացորդը հասանելի է։",
    "Open the buyer's profile on {venue} and create the first P2P order.": "Բացեք գնորդի պրոֆիլը {venue}-ում և ստեղծեք առաջին P2P պատվերը։",
    "Open the seller's profile on {venue}, create the P2P order, and pay using the selected method.": "Բացեք վաճառողի պրոֆիլը {venue}-ում, ստեղծեք P2P պատվերը և վճարեք ընտրված եղանակով։",
    "Before creating the order, compare the nickname and advertisement ID.": "Պատվեր ստեղծելուց առաջ համեմատեք մականունը և հայտարարության ID-ն։",
    "Check the current rate, order limits, and payment method on {venue}.": "Ստուգեք ընթացիկ փոխարժեքը, պատվերի սահմանաչափերը և վճարման եղանակը {venue}-ում։",
    "This bank fee is only an estimate. Check the final bank fee before sending.": "Այս բանկային միջնորդավճարը միայն գնահատական է։ Ուղարկելուց առաջ ստուգեք վերջնական միջնորդավճարը։",
    "Use only the payment details shown inside the order. After sending, mark the order as paid.": "Օգտագործեք միայն պատվերի մեջ նշված վճարման տվյալները։ Ուղարկելուց հետո նշեք պատվերը որպես վճարված։",
    "Release the asset only after you personally see that the payment has arrived.": "Ակտիվը փոխանցեք միայն այն բանից հետո, երբ անձամբ համոզվեք, որ վճարումը հասել է։",
    "Swap {from} for {to} via {venue}": "Փոխարկել {from}-ը {to}-ի {venue}-ով",
    "After you get {from}, send it to {venue} and exchange it for {to}.": "{from}-ը ստանալուց հետո ուղարկեք այն {venue} և փոխարկեք {to}-ի։",
    "Send {from} to {venue} first, then exchange it for {to}.": "Նախ ուղարկեք {from}-ը {venue}, ապա փոխարկեք {to}-ի։",
    "Before sending, check the asset, the receiving asset, and the exact network.": "Ուղարկելուց առաջ ստուգեք ակտիվը, ստացող ակտիվը և ճշգրիտ ցանցը։",
    "Check the current rate, provider fee, quote expiry, and any address, memo, or tag requirement.": "Ստուգեք ընթացիկ փոխարժեքը, մատակարարի միջնորդավճարը, հաշվարկի ժամկետը և հասցեի, memo-ի կամ tag-ի պահանջները։",
    "Wait until the new balance appears before considering this step finished.": "Այս քայլը ավարտված համարեք միայն նոր մնացորդի հայտնվելուց հետո։",
    "Transfer {asset} to {venue}": "Փոխանցել {asset}-ը {venue}",
    "Send the purchased asset from {from} to the deposit address on {to} before opening the next order.": "Հաջորդ պատվերը բացելուց առաջ գնված ակտիվը {from}-ից ուղարկեք {to}-ի ավանդի հասցեին։",
    "Copy the deposit address from {venue}. Choose the exact {network} network on both platforms.": "Պատճենեք ավանդի հասցեն {venue}-ից։ Երկու հարթակներում ընտրեք նույն՝ {network} ցանցը։",
    "First check that both platforms support the same asset and network. Then copy the deposit address from {venue}.": "Նախ ստուգեք, որ երկու հարթակներն էլ աջակցում են նույն ակտիվին և ցանցին։ Ապա պատճենեք ավանդի հասցեն {venue}-ից։",
    "Check the complete address, memo or tag if required, and the withdrawal fee before confirming.": "Հաստատելուց առաջ ստուգեք ամբողջական հասցեն, անհրաժեշտ memo-ն կամ tag-ը և դուրսբերման միջնորդավճարը։",
    "Wait until {venue} shows the deposit as received before continuing.": "Շարունակելուց առաջ սպասեք, մինչև {venue}-ը ցույց տա, որ ավանդը ստացվել է։",
    "Open the seller's profile on {venue} and create the order for the asset you want to receive.": "Բացեք վաճառողի պրոֆիլը {venue}-ում և ստեղծեք ձեր ստանալ ցանկացող ակտիվի պատվերը։",
    "Open the buyer's profile on {venue} and create a sell order using the selected payment method.": "Բացեք գնորդի պրոֆիլը {venue}-ում և ստեղծեք վաճառքի պատվեր՝ ստացման ընտրված եղանակով։",
    "Check the current rate, order limits, {thing}, and expected amount.": "Ստուգեք ընթացիկ փոխարժեքը, պատվերի սահմանաչափերը, {thing}-ը և ակնկալվող գումարը։",
    "This bank fee is only an estimate. Check the final fee before accepting the payout.": "Այս բանկային միջնորդավճարը միայն գնահատական է։ Վճարումը ընդունելուց առաջ ստուգեք վերջնական միջնորդավճարը։",
    "After the sale, check that the money has arrived in your account before considering the exchange finished.": "Վաճառքից հետո ստուգեք, որ գումարը հասել է ձեր հաշվին, և միայն այդ ժամանակ փոխանակումը համարեք ավարտված։",
    "Release the asset only after you personally see the payment in your bank or payment account.": "Ակտիվը փոխանցեք միայն այն բանից հետո, երբ անձամբ տեսնեք վճարումը ձեր բանկային կամ վճարային հաշվին։",
    "Important: rates, limits, and offers can change. Before sending money, check the provider or person, payment details, and network. Pay3Flow does not create orders or move money.": "Կարևոր է․ փոխարժեքները, սահմանաչափերը և առաջարկները կարող են փոխվել։ Գումար ուղարկելուց առաջ ստուգեք մատակարարին կամ անձին, վճարման տվյալներն ու ցանցը։ Pay3Flow-ը պատվերներ չի ստեղծում և գումար չի տեղափոխում։",
    "Rates, limits, and offers can change. Check the provider, payment details, and network before sending money. Pay3Flow does not create orders or move money.": "Փոխարժեքները, սահմանաչափերը և առաջարկները կարող են փոխվել։ Գումար ուղարկելուց առաջ ստուգեք մատակարարին, վճարման տվյալներն ու ցանցը։ Pay3Flow-ը պատվերներ չի ստեղծում և գումար չի տեղափոխում։",
    "Live dry quote from {provider}; execution and wallet compatibility are not verified.": "{provider}-ի ընթացիկ նախնական հաշվարկն է․ կատարումը և դրամապանակների համատեղելիությունը ստուգված չեն։",
    "Quoted exchanger: {description}.": "Հաշվարկում նշված փոխանակողը՝ {description}։",
    "Cross-network transfer requires the provider's deposit and withdrawal flow; confirm addresses, memos, network fees, and finality before sending.": "Ցանցերի միջև փոխանցումը պահանջում է մատակարարի մուտքագրման և դուրսբերման գործընթացը․ ուղարկելուց առաջ հաստատեք հասցեները, memo-ները, ցանցի վճարներն ու վերջնական ստացումը։",
    "Search estimate only: platform fees, account eligibility and execution are not verified.": "Սա միայն նախնական հաշվարկ է․ հարթակի միջնորդավճարները, հաշվի օգտագործման հնարավորությունը և կատարումը ստուգված չեն։",
    "Cross-venue route requires an asset transfer; network fee and compatible network are not included.": "Տարբեր հարթակներով ուղղությունը պահանջում է ակտիվի փոխանցում․ ցանցի վճարը և համատեղելի ցանցը հաշվարկում ներառված չեն։",
    "The selected sender bank could not be verified because the venue returned an opaque payment-method ID.": "Ուղարկողի ընտրված բանկը չհաջողվեց ստուգել․ հարթակը վերադարձրել է վճարման եղանակի անհասկանալի ID։",
    "The selected recipient bank could not be verified because the venue returned an opaque payment-method ID.": "Ստացողի ընտրված բանկը չհաջողվեց ստուգել․ հարթակը վերադարձրել է վճարման եղանակի անհասկանալի ID։",
    "At least one selected venue does not expose a public deep-link for this advertisement. Verify the advertiser ID and terms on the venue before sending money.": "Ընտրված հարթակներից առնվազն մեկը այս հայտարարության համար հանրային ուղիղ հղում չի տրամադրում։ Գումար ուղարկելուց առաջ հարթակում ստուգեք գովազդատուի ID-ն և պայմանները։",
    "Live fiat entry/exit offers plus a live dry cross-network quote; platform limits and execution are not verified.": "Մուտքի և ելքի ընթացիկ առաջարկներ և ցանցերի միջև փոխանցման նախնական հաշվարկ․ հարթակի սահմանաչափերն ու կատարումը ստուգված չեն։",
    "Confirm the source and destination networks, provider deposit address, memo/tag, network fee, and finality before sending.": "Ուղարկելուց առաջ հաստատեք սկզբնական և վերջնական ցանցերը, մատակարարի ավանդի հասցեն, memo/tag-ը, ցանցի վճարը և վերջնական ստացումը։",
    "Indicative direct-transfer quote; confirm the live rate, account eligibility, transfer limits, and recipient details with the provider before sending.": "Ուղղակի փոխանցման նախնական հաշվարկ է․ ուղարկելուց առաջ մատակարարի հետ ճշտեք ընթացիկ փոխարժեքը, հաշվի օգտագործման հնարավորությունը, փոխանցման սահմանաչափերը և ստացողի տվյալները։",
    "The selected sender payment method could not be verified by the venue.": "Հարթակը չկարողացավ ստուգել ուղարկողի վճարման ընտրված եղանակը։",
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
      let match = trimmed.match(/^(.*?) available via (digital wallet|bank transfer|cash)$/);
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
      match = trimmed.match(/^Convert (\S+) to (\S+)$/);
      if (match) translated = t("Convert {from} to {to}", { from: match[1], to: match[2] }, language);
      match = trimmed.match(/^Use the (.+) spot market on (.+)\. This is an exchange order book, so there is no P2P advertiser to contact\.$/);
      if (match) translated = t("Use the {pair} spot market on {venue}. This is an exchange order book, so there is no P2P advertiser to contact.", { pair: match[1], venue: match[2] }, language);
      match = trimmed.match(/^Confirm the pair converts (\S+) into (\S+)\.$/);
      if (match) translated = t("Confirm the pair converts {from} into {to}.", { from: match[1], to: match[2] }, language);
      match = trimmed.match(/^Open (\S+) and confirm it converts (\S+) into (\S+)\.$/);
      if (match) translated = t("Open {pair} and confirm it converts {from} into {to}.", { pair: match[1], from: match[2], to: match[3] }, language);
      match = trimmed.match(/^Conversion rate (.+)$/);
      if (match) translated = t("Conversion rate {rate}", { rate: match[1] }, language);
      match = trimmed.match(/^Open (\S+) on (.+)$/);
      if (match) translated = t("Open {pair} on {venue}", { pair: match[1], venue: match[2] }, language);
      match = trimmed.match(/^Complete the second conversion on (.+) only after the first trade has settled into your available balance\.$/);
      if (match) translated = t("Complete the second conversion on {venue} only after the first trade has settled into your available balance.", { venue: match[1] }, language);
      match = trimmed.match(/^Open the (buyer|seller)'s profile on (.+) and create the first P2P order\.$/);
      if (match) translated = t("Open {role}'s profile on {venue} and create the first P2P order.", { role: match[1], venue: match[2] }, language);
      match = trimmed.match(/^Open the seller's profile on (.+), create the P2P order, and pay with the selected payment method\.$/);
      if (match) translated = t("Open the seller's profile on {venue}, create the P2P order, and pay with the selected payment method.", { venue: match[1] }, language);
      match = trimmed.match(/^Open the buyer's profile on (.+) and create the sell order using the selected recipient payment method\.$/);
      if (match) translated = t("Open the buyer's profile on {venue} and create the sell order using the selected recipient payment method.", { venue: match[1] }, language);
      match = trimmed.match(/^Open the direct exchange on (.+), review the live quote, and complete the conversion in the provider flow\.$/);
      if (match) translated = t("Open the direct exchange on {venue}, review the live quote, and complete the conversion in the provider flow.", { venue: match[1] }, language);
      match = trimmed.match(/^Complete any login or verification required by (.+) and follow its (payment|transfer) instructions\.$/);
      if (match) translated = t(`Complete any login or verification required by {venue} and follow its ${match[2]} instructions.`, { venue: match[1] }, language);
      match = trimmed.match(/^Confirm the live rate, order limits, and payment method on (.+)\.$/);
      if (match) translated = t("Confirm the live rate, order limits, and payment method on {venue}.", { venue: match[1] }, language);
      match = trimmed.match(/^Transfer (\S+) to (.+)$/);
      if (match) translated = t("Transfer {asset} to {venue}", { asset: match[1], venue: match[2] }, language);
      match = trimmed.match(/^Move the purchased asset from (.+) to your deposit address on (.+) before opening the next P2P order\.$/);
      if (match) translated = t("Move the purchased asset from {from} to your deposit address on {to} before opening the next P2P order.", { from: match[1], to: match[2] }, language);
      match = trimmed.match(/^Copy the deposit address from (.+) and select the exact (.+) network on both venues\.$/);
      if (match) translated = t("Copy the deposit address from {venue} and select the exact {network} network on both venues.", { venue: match[1], network: match[2] }, language);
      match = trimmed.match(/^Confirm that both venues support the same asset and network, then copy the deposit address from (.+)\.$/);
      if (match) translated = t("Confirm that both venues support the same asset and network, then copy the deposit address from {venue}.", { venue: match[1] }, language);
      match = trimmed.match(/^Wait for (.+) to credit the deposit before continuing\.$/);
      if (match) translated = t("Wait for {venue} to credit the deposit before continuing.", { venue: match[1] }, language);
      match = trimmed.match(/^(Buy|Sell) (\S+) for (.+)$/);
      if (match) translated = t(`${match[1]} {asset} for {amount}`, { asset: match[2], amount: match[3] }, language);
      match = trimmed.match(/^Buy (\S+) with (.+)$/);
      if (match) translated = t("Buy {asset} with {bridge}", { asset: match[1], bridge: match[2] }, language);
      match = trimmed.match(/^Confirm the (\S+) balance and network before withdrawing\.$/);
      if (match) translated = t("Confirm the {asset} balance and network before withdrawing.", { asset: match[1] }, language);
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

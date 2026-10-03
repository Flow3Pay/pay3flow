<script lang="ts">
  import Header from "$lib/components/Header.svelte";
  import { locale, type Locale } from "$lib/i18n";

  type PolicySection = { title: string; paragraphs: string[]; list?: string[] };
  type PolicyCopy = {
    title: string;
    description: string;
    back: string;
    version: string;
    heading: string;
    intro: string;
    sections: PolicySection[];
    closing: string;
  };

  const copy: Record<Locale, PolicyCopy> = {
    ru: {
      title: "Правила использования — Pay3Flow",
      description: "Правила использования приложения Pay3Flow.",
      back: "← На главную",
      version: "Pay3Flow · версия от 3 октября 2026 года",
      heading: "Правила использования",
      intro: "Эти правила описывают условия доступа к Pay3Flow и его использования. Пользуясь приложением, вы соглашаетесь соблюдать их. Если вы не согласны с правилами, пожалуйста, не используйте приложение.",
      sections: [
        {
          title: "1. Что делает Pay3Flow",
          paragraphs: [
            "Pay3Flow — экспериментальный сервис поиска и сравнения маршрутов обмена между фиатными валютами и цифровыми активами. Он получает общедоступные предложения и котировки сторонних площадок и показывает расчётные варианты маршрута.",
            "Pay3Flow не является банком, брокером, биржей, обменником, платёжной организацией или вашим представителем. Сервис сам не размещает ордера, не связывается с контрагентами, не принимает и не хранит деньги или цифровые активы и не гарантирует исполнение обмена. Если отдельная функция предлагает подключить кошелёк или подписать действие, его детали отображаются в кошельке или соответствующем интерфейсе; проверяйте их до подтверждения."
          ]
        },
        {
          title: "2. Котировки и сторонние площадки",
          paragraphs: [
            "Результаты — информационные оценки, а не оферта, финансовая рекомендация или обещание доступности сделки. Курсы, комиссии, лимиты, способы оплаты, сроки и условия могут быстро меняться. Источники могут быть недоступны, возвращать неполные данные или показывать предложения, которые уже нельзя исполнить.",
            "Перед любой операцией самостоятельно откройте сайт выбранной площадки и проверьте контрагента, реквизиты, сумму, курс, комиссии, лимиты, срок действия котировки, актив, сеть, адрес и обязательные memo или tag. Условия сторонней площадки регулируются её собственными правилами. Pay3Flow не контролирует такие площадки и не отвечает за их действия или доступность."
          ]
        },
        {
          title: "3. Ваша ответственность",
          paragraphs: ["Используя Pay3Flow, вы подтверждаете, что:"],
          list: [
            "вам разрешено пользоваться сервисом по законам, применимым к вам;",
            "вы сами проверяете законность и налоговые последствия каждой операции в своей юрисдикции;",
            "вы используете только принадлежащие вам средства или средства, которыми вправе распоряжаться;",
            "вы проверяете все условия и данные операции до перевода или подписи транзакции;",
            "вы понимаете риск потери средств, задержки, ошибочного перевода, волатильности курса и сбоя стороннего сервиса."
          ]
        },
        {
          title: "4. Допустимое использование",
          paragraphs: ["Запрещается использовать приложение, чтобы:"],
          list: [
            "нарушать закон, права других лиц или правила сторонних площадок;",
            "совершать мошенничество, отмывание денег, финансирование терроризма или обход санкций;",
            "получать несанкционированный доступ к сервису, мешать его работе или обходить технические ограничения;",
            "передавать вредоносный код, сканировать инфраструктуру без разрешения либо перегружать её автоматическими запросами;",
            "использовать результаты сервиса для введения других лиц в заблуждение."
          ]
        },
        {
          title: "5. Доступность и изменения",
          paragraphs: ["Приложение предоставляется в текущем виде. Мы можем менять, приостанавливать или прекращать отдельные функции, источники данных или доступ к сервису. Мы стремимся поддерживать работу приложения, но не обещаем бесперебойную доступность, точность или полноту результатов."]
        },
        {
          title: "6. Ограничение ответственности",
          paragraphs: ["В пределах, допускаемых применимым правом, Pay3Flow не отвечает за решения, принятые на основе отображённых оценок, операции между вами и третьими лицами, действия внешних площадок, потерю средств, сбои сетей или недоступность данных. Ничто в этих правилах не исключает ответственность, которую нельзя исключить по закону."]
        },
        {
          title: "7. Обновление правил",
          paragraphs: ["Мы можем обновлять эти правила. Новая редакция публикуется на этой странице вместе с датой обновления и применяется с момента публикации, если в ней не указано иное. Продолжая пользоваться приложением после обновления, вы принимаете новую редакцию."]
        }
      ],
      closing: "Если вы не уверены в условиях или законности операции, не совершайте её до самостоятельной проверки."
    },
    en: {
      title: "Usage Policy — Pay3Flow",
      description: "Terms for using the Pay3Flow application.",
      back: "← Home",
      version: "Pay3Flow · version dated October 3, 2026",
      heading: "Usage Policy",
      intro: "These rules describe the terms for accessing and using Pay3Flow. By using the application, you agree to follow them. If you do not agree, please do not use the application.",
      sections: [
        {
          title: "1. What Pay3Flow does",
          paragraphs: [
            "Pay3Flow is an experimental service for finding and comparing exchange routes between fiat currencies and digital assets. It retrieves publicly available offers and quotes from third-party platforms and displays estimated route options.",
            "Pay3Flow is not a bank, broker, exchange, exchanger, payment institution, or your agent. The service does not place orders, contact counterparties, accept or hold money or digital assets, or guarantee that an exchange will be completed. If a feature offers a wallet connection or asks you to sign an action, its details are shown in your wallet or the relevant interface; review them before confirming."
          ]
        },
        {
          title: "2. Quotes and third-party platforms",
          paragraphs: [
            "Results are informational estimates, not an offer, financial recommendation, or promise that a transaction is available. Rates, fees, limits, payment methods, timing, and terms may change quickly. Sources may be unavailable, return incomplete data, or show offers that can no longer be completed.",
            "Before any transaction, open the selected platform yourself and verify the counterparty, payment details, amount, rate, fees, limits, quote validity, asset, network, address, and any required memo or tag. Third-party platforms are governed by their own terms. Pay3Flow does not control those platforms and is not responsible for their actions or availability."
          ]
        },
        {
          title: "3. Your responsibilities",
          paragraphs: ["By using Pay3Flow, you confirm that:"],
          list: [
            "you are allowed to use the service under the laws that apply to you;",
            "you independently check the legality and tax consequences of each transaction in your jurisdiction;",
            "you use only funds that belong to you or that you are authorized to manage;",
            "you verify all transaction terms and details before transferring funds or signing a transaction;",
            "you understand the risks of loss, delay, mistaken transfers, rate volatility, and third-party service failures."
          ]
        },
        {
          title: "4. Acceptable use",
          paragraphs: ["You must not use the application to:"],
          list: [
            "break the law, violate another person's rights, or breach a third-party platform's rules;",
            "commit fraud, launder money, finance terrorism, or evade sanctions;",
            "gain unauthorized access to the service, disrupt it, or bypass technical restrictions;",
            "transmit malicious code, scan infrastructure without permission, or overload it with automated requests;",
            "use the service's results to mislead other people."
          ]
        },
        {
          title: "5. Availability and changes",
          paragraphs: ["The application is provided as is. We may change, suspend, or discontinue individual features, data sources, or access to the service. We work to keep the application available, but do not promise uninterrupted access or that results will be accurate or complete."]
        },
        {
          title: "6. Limitation of liability",
          paragraphs: ["To the extent permitted by applicable law, Pay3Flow is not responsible for decisions made based on displayed estimates, transactions between you and third parties, actions of external platforms, loss of funds, network failures, or unavailable data. Nothing in these rules excludes liability that cannot be excluded by law."]
        },
        {
          title: "7. Changes to these rules",
          paragraphs: ["We may update these rules. The revised version will be posted on this page with its update date and will apply from publication unless stated otherwise. If you continue using the application after an update, you accept the revised version."]
        }
      ],
      closing: "If you are unsure about the terms or legality of a transaction, do not proceed until you have checked independently."
    },
    hy: {
      title: "Օգտագործման կանոններ — Pay3Flow",
      description: "Pay3Flow հավելվածի օգտագործման կանոնները։",
      back: "← Գլխավոր էջ",
      version: "Pay3Flow · տարբերակ՝ 3 հոկտեմբերի, 2026 թ.",
      heading: "Օգտագործման կանոններ",
      intro: "Այս կանոնները սահմանում են Pay3Flow մուտք գործելու և այն օգտագործելու պայմանները։ Հավելվածն օգտագործելով՝ դուք համաձայնում եք պահպանել դրանք։ Եթե համաձայն չեք, խնդրում ենք չօգտագործել հավելվածը։",
      sections: [
        {
          title: "1. Ինչ է անում Pay3Flow-ը",
          paragraphs: [
            "Pay3Flow-ը փորձնական ծառայություն է, որը գտնում և համեմատում է ֆիատ արժույթների և թվային ակտիվների փոխանակման ուղիները։ Այն ստանում է երրորդ կողմի հարթակների հանրամատչելի առաջարկներն ու գնանշումները և ցուցադրում է ուղիների մոտավոր տարբերակները։",
            "Pay3Flow-ը բանկ, բրոքեր, բորսա, փոխանակման կետ, վճարային կազմակերպություն կամ ձեր ներկայացուցիչը չէ։ Ծառայությունն ինքնուրույն պատվերներ չի տեղադրում, չի կապվում գործարքի մյուս կողմի հետ, չի ընդունում կամ պահում գումար կամ թվային ակտիվներ և չի երաշխավորում փոխանակման կատարումը։ Եթե որևէ գործառույթ առաջարկում է միացնել դրամապանակը կամ ստորագրել գործողություն, դրա մանրամասները ցուցադրվում են դրամապանակում կամ համապատասխան միջերեսում․ հաստատելուց առաջ ստուգեք դրանք։"
          ]
        },
        {
          title: "2. Գնանշումներ և երրորդ կողմի հարթակներ",
          paragraphs: [
            "Արդյունքները տեղեկատվական գնահատականներ են, այլ ոչ թե առաջարկ, ֆինանսական խորհրդատվություն կամ գործարքի հասանելիության խոստում։ Փոխարժեքները, վճարները, սահմանաչափերը, վճարման եղանակները, ժամկետներն ու պայմանները կարող են արագ փոխվել։ Տվյալների աղբյուրները կարող են անհասանելի լինել, թերի տվյալներ վերադարձնել կամ ցուցադրել առաջարկներ, որոնք այլևս հնարավոր չէ կատարել։",
            "Գործարքից առաջ ինքնուրույն բացեք ընտրված հարթակի կայքը և ստուգեք գործարքի մյուս կողմին, վճարման տվյալները, գումարը, փոխարժեքը, վճարները, սահմանաչափերը, գնանշման վավերականության ժամկետը, ակտիվը, ցանցը, հասցեն և անհրաժեշտ memo-ն կամ tag-ը։ Երրորդ կողմի հարթակի պայմանները կարգավորվում են դրա սեփական կանոններով։ Pay3Flow-ը չի վերահսկում այդ հարթակները և պատասխանատու չէ դրանց գործողությունների կամ հասանելիության համար։"
          ]
        },
        {
          title: "3. Ձեր պարտականությունները",
          paragraphs: ["Օգտագործելով Pay3Flow-ը՝ դուք հաստատում եք, որ՝"],
          list: [
            "ձեզ թույլատրվում է օգտվել ծառայությունից ձեր նկատմամբ կիրառելի օրենքների համաձայն,",
            "ձեր իրավասության տարածքում ինքնուրույն ստուգում եք յուրաքանչյուր գործարքի օրինականությունն ու հարկային հետևանքները,",
            "օգտագործում եք միայն ձեզ պատկանող կամ ձեր տնօրինման իրավունքի տակ գտնվող միջոցները,",
            "փոխանցում կատարելուց կամ գործարք ստորագրելուց առաջ ստուգում եք դրա բոլոր պայմաններն ու տվյալները,",
            "գիտակցում եք միջոցների կորստի, ուշացման, սխալ փոխանցման, փոխարժեքի տատանման և երրորդ կողմի ծառայության խափանման ռիսկերը։"
          ]
        },
        {
          title: "4. Թույլատրելի օգտագործում",
          paragraphs: ["Արգելվում է հավելվածն օգտագործել՝"],
          list: [
            "օրենքը, այլ անձանց իրավունքները կամ երրորդ կողմի հարթակների կանոնները խախտելու համար,",
            "խարդախություն կատարելու, փողերի լվացման, ահաբեկչության ֆինանսավորման կամ պատժամիջոցները շրջանցելու համար,",
            "ծառայություն չարտոնված մուտք ստանալու, դրա աշխատանքը խաթարելու կամ տեխնիկական սահմանափակումները շրջանցելու համար,",
            "վնասաբեր կոդ փոխանցելու, առանց թույլտվության ենթակառուցվածքը զննելու կամ այն ավտոմատ հարցումներով ծանրաբեռնելու համար,",
            "ծառայության արդյունքներն այլ անձանց մոլորեցնելու նպատակով օգտագործելու համար։"
          ]
        },
        {
          title: "5. Հասանելիություն և փոփոխություններ",
          paragraphs: ["Հավելվածը տրամադրվում է իր ներկա վիճակով։ Մենք կարող ենք փոխել, կասեցնել կամ դադարեցնել առանձին գործառույթներ, տվյալների աղբյուրներ կամ ծառայության հասանելիությունը։ Մենք ձգտում ենք պահպանել հավելվածի աշխատանքը, սակայն չենք երաշխավորում անխափան հասանելիություն կամ արդյունքների ճշգրտությունն ու ամբողջականությունը։"]
        },
        {
          title: "6. Պատասխանատվության սահմանափակում",
          paragraphs: ["Կիրառելի օրենքով թույլատրված սահմաններում Pay3Flow-ը պատասխանատվություն չի կրում ցուցադրված գնահատականների հիման վրա ընդունված որոշումների, ձեր և երրորդ անձանց միջև գործարքների, արտաքին հարթակների գործողությունների, միջոցների կորստի, ցանցերի խափանումների կամ տվյալների անհասանելիության համար։ Այս կանոններից ոչինչ չի բացառում այն պատասխանատվությունը, որը օրենքով չի կարող բացառվել։"]
        },
        {
          title: "7. Կանոնների փոփոխություն",
          paragraphs: ["Մենք կարող ենք թարմացնել այս կանոնները։ Նոր տարբերակը՝ թարմացման ամսաթվով, կհրապարակվի այս էջում և ուժի մեջ կմտնի հրապարակման պահից, եթե այլ բան նշված չէ։ Թարմացումից հետո հավելվածը շարունակելով օգտագործել՝ դուք ընդունում եք նոր տարբերակը։"]
        }
      ],
      closing: "Եթե վստահ չեք գործարքի պայմանների կամ օրինականության հարցում, նախ ինքնուրույն ստուգեք դրանք և միայն դրանից հետո շարունակեք։"
    }
  };

  $: activeLocale = $locale;
  $: policy = copy[activeLocale];
</script>

<svelte:head>
  <title>{policy.title}</title>
  <meta name="description" content={policy.description} />
</svelte:head>

<div class="appShell">
  <Header />
  <main class="policyPage" lang={activeLocale}>
    <a class="backLink" href="/">{policy.back}</a>
    <article class="policy">
      <p class="eyebrow">{policy.version}</p>
      <h1>{policy.heading}</h1>
      <p class="lead">{policy.intro}</p>

      {#each policy.sections as section}
        <section>
          <h2>{section.title}</h2>
          {#each section.paragraphs as paragraph}
            <p>{paragraph}</p>
          {/each}
          {#if section.list}
            <ul>
              {#each section.list as item}
                <li>{item}</li>
              {/each}
            </ul>
          {/if}
        </section>
      {/each}

      <p class="closing">{policy.closing}</p>
    </article>
  </main>
  <footer class="siteFooter"><span>Pay3Flow</span><span>{policy.heading}</span></footer>
</div>

<style>
  .policyPage {
    width: min(920px, calc(100% - 48px));
    margin: 32px auto 0;
  }

  .backLink {
    display: inline-flex;
    margin-bottom: 18px;
    color: var(--color-text-soft);
    font-size: 13px;
    font-weight: 700;
  }

  .backLink:hover { color: var(--color-text); }

  .policy {
    padding: clamp(24px, 5vw, 56px);
    border: 1px solid var(--color-border);
    border-radius: 18px;
    background: var(--color-paper);
    box-shadow: var(--shadow-card);
  }

  .eyebrow {
    color: var(--color-text-faint);
    font-size: 11px;
    font-weight: 750;
    letter-spacing: 0.07em;
    text-transform: uppercase;
  }

  h1 {
    margin-top: 12px;
    font-size: clamp(30px, 5vw, 46px);
    line-height: 1.12;
    letter-spacing: -0.045em;
  }

  .lead {
    margin-top: 18px;
    color: var(--color-text-soft);
    font-size: 16px;
    line-height: 1.75;
  }

  section { margin-top: 34px; }

  h2 {
    margin-bottom: 10px;
    font-size: 18px;
    line-height: 1.4;
    letter-spacing: -0.02em;
  }

  section p,
  li,
  .closing {
    color: var(--color-text-soft);
    font-size: 14px;
    line-height: 1.8;
  }

  section p + p { margin-top: 10px; }

  ul {
    display: grid;
    gap: 7px;
    margin: 10px 0 10px 22px;
  }

  li { padding-left: 3px; }

  .closing {
    margin-top: 36px;
    padding-top: 20px;
    border-top: 1px solid var(--color-border);
    font-weight: 700;
  }

  @media (max-width: 560px) {
    .policyPage { width: calc(100% - 28px); margin-top: 22px; }
    .policy { border-radius: 14px; }
    .lead { font-size: 15px; }
    section p, li, .closing { font-size: 13px; }
  }
</style>

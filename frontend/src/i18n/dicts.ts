export type Lang = "en" | "ru";

export type TagColor = "orange" | "purple" | "blue";

export type Seg = string | [string, TagColor];

export interface LinkItem {
  href: string;
  label: string;
}

export interface FeatureCard {
  title: string;
  text: string;
  link: string;
  href: string;
}

export interface StatItem {
  value: string;
  label: string;
}

export interface Dict {
  header: {
    navTrigger: string;
    ariaNav: string;
    nav: { exchange: string; payouts: string; pricing: string };
    launch: string;
    wallet: { connect: string; disconnect: string };
    mobileMenu: { open: string; close: string; ariaLabel: string };
  };
  hero: {
    title: string;
  };
  features: {
    headline: Seg[];
    cards: FeatureCard[];
  };
  why: {
    label: string;
    heading: string;
    sub: string;
    cards: { title: string; text: string; href: string }[];
  };
  business: {
    label: string;
    heading: string;
    sub: string;
    cta: string;
    ctaHref: string;
    stats: StatItem[];
  };
  footer: {
    blurb: string;
    columns: { title: string; links: LinkItem[] }[];
    rights: string;
    version: string;
  };
}

export const dicts: Record<Lang, Dict> = {
  en: {
    header: {
      navTrigger: "Sections",
      ariaNav: "Main menu",
      nav: {
        exchange: "Exchange",
        payouts: "Payouts",
        pricing: "Pricing",
      },
      launch: "Exchange",
      wallet: {
        connect: "Connect wallet",
        disconnect: "Disconnect wallet",
      },
      mobileMenu: {
        open: "Open menu",
        close: "Close menu",
        ariaLabel: "Mobile menu",
      },
    },
    hero: {
      title: "Don't get fooled.",
    },
    features: {
      headline: [
        "Pay3Flow moves money ",
        ["directly", "orange"],
        " between people — so you ",
        ["pay less", "purple"],
        " and ",
        ["worry less", "blue"],
        ".",
      ],
      cards: [
        {
          title: "Pay3Flow Network",
          text: "A payment network with direct routes between people and businesses — no chains of intermediaries.",
          link: "Start paying",
          href: "/swap",
        },
        {
          title: "Pay3Flow Exchange",
          text: "Convert and send at market rates with a transparent fee — no hidden cuts.",
          link: "Exchange now",
          href: "/swap",
        },
      ],
    },
    why: {
      label: "Why Pay3Flow",
      heading: "Payments should never make you wait or overpay",
      sub: "Direct routes keep money moving — faster and cheaper than the usual channels.",
      cards: [
        {
          title: "Direct routes",
          text: "Money goes straight from sender to receiver — without long chains of banks.",
          href: "/#how",
        },
        {
          title: "Fair rates",
          text: "Conversion at market rates with a transparent fee from 0.5% — no hidden add-ons.",
          href: "/#business",
        },
        {
          title: "Fast delivery",
          text: "Most routes settle within a day, not a week of bank days.",
          href: "/#business",
        },
      ],
    },
    business: {
      label: "For business",
      heading: "Payment routes for your product",
      sub: "Connect Pay3Flow to your service and pay only for successful payments.",
      cta: "Become a client",
      ctaHref: "#business",
      stats: [
        { value: "0.5%", label: "fee from" },
        { value: "24 h", label: "average settlement" },
        { value: "40+", label: "countries" },
      ],
    },
    footer: {
      blurb:
        "Pay3Flow is an open network of direct payment routes — convert and deliver payments straight to the recipient, no middlemen.",
      columns: [
        {
          title: "Product",
          links: [
            { href: "/swap", label: "Exchange" },
            { href: "/#how", label: "Payouts" },
            { href: "/#business", label: "Pricing" },
          ],
        },
        {
          title: "Learn",
          links: [
            { href: "/#how", label: "How it works" },
            { href: "/#business", label: "FAQ" },
            { href: "/#how", label: "Articles" },
          ],
        },
        {
          title: "Company",
          links: [
            { href: "/#how", label: "About" },
            { href: "/#business", label: "For business" },
            { href: "/#top", label: "Security" },
            { href: "/#top", label: "Legal" },
          ],
        },
        {
          title: "Community",
          links: [
            { href: "#top", label: "Telegram" },
            { href: "#top", label: "X / Twitter" },
            { href: "#top", label: "Blog" },
          ],
        },
      ],
      rights: "© Pay3Flow — 2026",
      version: "v0.1.0",
    },
  },

  ru: {
    header: {
      navTrigger: "Разделы",
      ariaNav: "Основное меню",
      nav: {
        exchange: "Обмен",
        payouts: "Выплаты",
        pricing: "Тарифы",
      },
      launch: "Обмен",
      wallet: {
        connect: "Подключить кошелёк",
        disconnect: "Отключить кошелёк",
      },
      mobileMenu: {
        open: "Открыть меню",
        close: "Закрыть меню",
        ariaLabel: "Мобильное меню",
      },
    },
    hero: {
      title: "Не дайте себя обмануть",
    },
    features: {
      headline: [
        "Pay3Flow переводит деньги ",
        ["напрямую", "orange"],
        " между людьми — вы ",
        ["платите меньше", "purple"],
        " и ",
        ["меньше тревожитесь", "blue"],
        ".",
      ],
      cards: [
        {
          title: "Сеть Pay3Flow",
          text: "Платёжная сеть с прямыми маршрутами между людьми и компаниями — без цепочек посредников.",
          link: "Начать платить",
          href: "/swap",
        },
        {
          title: "Обмен Pay3Flow",
          text: "Конвертируйте и отправляйте по рыночному курсу с прозрачной комиссией — без скрытых наценок.",
          link: "Обменять сейчас",
          href: "/swap",
        },
      ],
    },
    why: {
      label: "Почему Pay3Flow",
      heading: "Платежи не должны заставлять вас ждать или переплачивать",
      sub: "Прямые маршруты двигают деньги быстрее и дешевле привычных каналов.",
      cards: [
        {
          title: "Прямые маршруты",
          text: "Деньги идут напрямую от отправителя к получателю — без длинных цепочек банков.",
          href: "/#how",
        },
        {
          title: "Честные курсы",
          text: "Конвертация по рыночному курсу с прозрачной комиссией от 0,5% — без скрытых наценок.",
          href: "/#business",
        },
        {
          title: "Быстрая доставка",
          text: "Большинство маршрутов зачисляется в течение дня, а не за неделю банковских дней.",
          href: "/#business",
        },
      ],
    },
    business: {
      label: "Для бизнеса",
      heading: "Платёжные маршруты для вашего продукта",
      sub: "Подключите Pay3Flow к своему сервису и платите только за успешные платежи.",
      cta: "Стать клиентом",
      ctaHref: "#business",
      stats: [
        { value: "0,5%", label: "комиссия от" },
        { value: "24 ч", label: "среднее зачисление" },
        { value: "40+", label: "стран" },
      ],
    },
    footer: {
      blurb:
        "Pay3Flow — открытая сеть прямых платёжных маршрутов. Конвертация и доставка платежей напрямую получателю без посредников.",
      columns: [
        {
          title: "Продукт",
          links: [
            { href: "/swap", label: "Обмен" },
            { href: "/#how", label: "Выплаты" },
            { href: "/#business", label: "Тарифы" },
          ],
        },
        {
          title: "Обучение",
          links: [
            { href: "/#how", label: "Как это работает" },
            { href: "/#business", label: "FAQ" },
            { href: "/#how", label: "Статьи" },
          ],
        },
        {
          title: "Компания",
          links: [
            { href: "/#how", label: "О нас" },
            { href: "/#business", label: "Для бизнеса" },
            { href: "/#top", label: "Безопасность" },
            { href: "/#top", label: "Юридическая информация" },
          ],
        },
        {
          title: "Сообщество",
          links: [
            { href: "#top", label: "Telegram" },
            { href: "#top", label: "X / Twitter" },
            { href: "#top", label: "Блог" },
          ],
        },
      ],
      rights: "© Pay3Flow — 2026",
      version: "v0.1.0",
    },
  },
};
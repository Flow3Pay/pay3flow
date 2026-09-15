# fmatch offer — как представить эквайера

Эквайер в fmatch = FEP-0837 offer. В нашей модели это «паспорт эквайера»:
кто он, какими ресурсами/валютами/гео оперирует, какая комиссия, лимиты,
статус. fmatch хранит это в `marketplace_proposals` (`purpose='offer'`) и
индексирует в Typesense для матчинга по ресурсу/действию.

## Жёсткое правило: сначала Follow

Офер заэквайера не принимается, пока у его актора нет активной
`marketplace_subscriptions` с теми же `resourceConformsTo`+`action`
(проверка `has_active_marketplace_subscription` в `entry.rs`).

Последовательность:

1. Поднять наш actor-документ (URL из `actor/`), в `attachment` указать
   capability: `resourceConformsTo`, `action`, `purpose`.
2. `POST /inbox/actra` `Follow` c `actor` = наш actor IRI. fmatch фетчит
   документ, читает capability-вложение и создаёт подписку.
3. Только после этого слать `Proposal` c `purpose:"offer"`.

## Схема offer (эквайер)

```json
{
  "@context": [
    "https://www.w3.org/ns/activitystreams",
    "https://w3id.org/fep/0837"
  ],
  "type": "Proposal",
  "id": "https://pay3flow.dev/acquirers/stripe#offer",
  "purpose": "offer",
  "attributedTo": "https://pay3flow.dev/actor/pay3flow",
  "name": "Stripe (US, USD/EUR, 3.1% card)",
  "content": "geo:US|EU; currencies:USD,EUR; fee: 3.1% + 0.50$; limits: min 1, max 100000 USD",
  "publishes": {
    "type": "Intent",
    "id": "https://pay3flow.dev/acquirers/stripe#intent",
    "action": "deliverService",
    "resourceConformsTo": "https://pay3flow.dev/marketplace/resources/acquiring",
    "resourceQuantity": {"hasUnit": "one", "hasNumericalValue": "1"}
  },
  "to": ["https://www.w3.org/ns/activitystreams#Public"]
}
```

Валидируемые поля — только подмножество ниже; остальное для нас (комиссия,
гео, лимиты) едет в `name`/`content` как структурированный текст и в
`source_activity` original-пейлода. Файншенс хранится дословно в
`marketplace_proposals.content`.

## Семантика полей под запросы матчинга

| Понятие | Поле | Значение для эквайера |
|---|---|---|
| Предмет сделки | `publishes.resourceConformsTo` | resource-контракт эквайринга, напр. `https://pay3flow.dev/marketplace/resources/acquiring` |
| Действие | `publishes.action` | `deliverService` |
| Единица | `publishes.resourceQuantity.hasUnit` | `one` / `currencyAmount` |
| Аудитория | `to` | `@Public` (offer виден всем) |
| Описание | `name`, `content` | человекочитаемо; наши доп. поля: валюта, гео, комиссия, лимиты, статус |

## Resource-контракт

`resourceConformsTo` у **запроса** и **оффера** должен совпадать (или оба
`/marketplace/resources/` на одном path — fmatch сравнивает по path без хоста,
см. `active_matching_offers_page`). Раз значение у нас своё — вводим единый
слот:

```
https://pay3flow.dev/marketplace/resources/acquiring
```

Все эквайеры и все платёжные запросы идут через этот один ресурс; различие
между эквайерами — в содержимом `content` (гео/валюта) и в ранжировании.
Фильтрации по валюте/гео у fmatch «как есть» нет — это забота нашего backend.

## Примечания

- `purpose='offer'` не проверяет `verify_resource_conforms_to`, подписка
  обязательна.
- Оффер должен быть валидным JSON-LD: `@context` с ActivityStreams и FEP-0837.
- Обновление эквайера = повторный POST с тем же `proposal.id` (upsert).
- Выключить эквайер: пометить `status` у нас (fmatch не имеет endpoint
  deactivate; `active_matching_offers_page` фильтрует `status='active'`).
- Идемпотентность: повторный offer c тем же `id` — `status:"exists"` / upsert.
# fmatch API — мини-заметка

Fmatch — ActivityPub-матчер, источник истины «что умеет юзер/сервис» — это
`marketplace_subscriptions` и `marketplace_proposals` в его Postgres. HTTP-пути —
транспорт ActivityPub (inbox/outbox/actor), а не REST-ресурсы.

Локально: `http://localhost:7277` (postgres :5433, typesense :8108).

## Поверхности

| Путь | Метод | Что делает |
|------|-------|------------|
| `/health` | GET | `{"status":"ok"}` |
| `/typesense/health` | GET | здоровье typesense |
| `/actor`, `/actors` | GET | список акторов (`OrderedCollection`) |
| `/actor/:handle` | GET | actor-документ (ActivityStreams) |
| `/inbox`, `/inbox/:handle` | POST | shared / per-actor inbox (приём activities) |
| `/outbox/:handle` | GET | pending-задачи (`OrderedCollection` из activities) |
| `/follow` | POST | подписаться на удалённый актор (от имени fmatch) |
| `/.well-known/webfinger?resource=acct:...` | GET | webfinger-резолюция |
| `/.well-known/nodeinfo`, `/nodeinfo/:ver` | GET | nodeinfo 2.0/2.1 |
| `/marketplace/resources/:name` | GET | описание resource-спецификации |
| `/activitypub/object?iri=...` | GET | fetch удалённого AP-объекта |

Базовый актор: `actra` (`https://<domain>/actor/actra`), shared inbox `/inbox/actra`.

## Принимаемые activity-типы (валидация `validate_marketplace_inbox_entry`)

- `Follow` — подписка. fmatch fetch-ит актор из `Follow.actor`, создаёт
  `marketplace_subscriptions`.
- `Proposal`, `Note` — FEP-0837 предложение (`purpose=offer|request`).
- `Create` / `Offer` с `object` = `Proposal` / `Note` / `OrderedCollection` /
  ForgeFed `Ticket|Issue|MergeRequest|PullRequest|Patch`.
- `Update(Proposal)` / `Update(Agreement)`.
- `OfferAgreement` / `AcceptAgreement` / `RejectAgreement` — settlement.
- `Accept` / `Reject` / `Undo` — settlement (ack).

## Как подать запрос (наш сценарий: платёжный запрос)

`POST /inbox/actra` с JSON-LD. Минимум для FEP-0837 proposal (`purpose=request`):

```json
{
  "@context": [
    "https://www.w3.org/ns/activitystreams",
    "https://w3id.org/fep/0837"
  ],
  "type": "Proposal",
  "id": "https://pay3flow.local/proposals/req-1",
  "purpose": "request",
  "attributedTo": "https://pay3flow.local/actor/pay3flow",
  "name": "Send 100 USD to TR account",
  "content": "100 USD, from US, to TR, bank transfer",
  "publishes": {
    "type": "Intent",
    "id": "https://pay3flow.local/proposals/req-1#intent",
    "action": "deliverService",
    "resourceConformsTo": "https://pay3flow.dev/marketplace/resources/acquiring",
    "resourceQuantity": {"hasUnit": "one", "hasNumericalValue": "1"}
  },
  "to": ["https://pay3flow.local/actor/pay3flow"]
}
```

Требования валидации (`validate_fep0837_proposal`):
- `@context` включает ActivityStreams **и** `https://w3id.org/fep/0837`;
- `proposal.id`, `type ∈ {Proposal,Note}`, `purpose ∈ {offer,request}`,
  `attributedTo`, `to`;
- `publishes` — `Intent` c `id`, `action ∈ {deliverService,transfer}`,
  `resourceConformsTo` (http/https), `resourceQuantity.hasUnit`;
- необязательно: количественники как строки (`hasNumericalValue: "0.89"`).

## Ответ fmatch

Fmatch отвечает ActivityPub-объектом, **не** REST-обёрткой:
- успех матчинга → `Offer(Agreement)` + блок `candidates` (ранжированный
  список; у каждого — price/латентность/качество);
- промежуточные → `Accept` с `status`;
- нет кандидатов → `Reject` (`status=no-candidates`, HTTP 503);
- падение исполнения → `Reject` (`execution-failed`, HTTP 502).

Внутренний envelope routing (`delivery/chain`) наружу не отдаётся.

## Маршрут результата

Для асинхронного результата в запросе указываем `resultInbox` (наш `/inbox`);
fmatch доставит `Offer(Agreement)`-результат туда через `candidate_result_inbox_url`
(`/inbox/actra` по умолчанию). Команды:
`new`/`candidates`/`findbestpath` — только список кандидатов без исполнения;
`execute[:N]` — зафиксировать кандидата N.
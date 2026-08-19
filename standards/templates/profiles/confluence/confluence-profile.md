---
title: Профиль платформы: Confluence
document_type: standard
status: maintained
scope: workspace
owner: workspace-owner
created: 2026-08-19
updated: 2026-08-19
---

# Профиль платформы: Confluence

Реализация [контракта формы](../../template-contract.md) для Confluence. Здесь описано
только то, чем эта платформа отличается от абстрактного требования; сами
требования — в контракте, и профиль их не повторяет и не переопределяет.

## Механика платформы

| Требование контракта | Чем обеспечено в Confluence |
|---|---|
| §1 Заголовок задаётся средствами платформы | Page Title — единственный H1. Тело не содержит повторяющий `# Заголовок` и начинает содержательные разделы с H2 |
| §2 Блок метаданных в теле | Обычная таблица в начале тела страницы; служебные свойства площадки её не заменяют |
| §6 Нет самоссылки | Адрес даёт сама страница; `canonical_url` в теле не хранится |
| §7 История ревизий для документов без версии | Встроенная Confluence version history плюс поле «Обновлено». Документы с объявленной версией ссылаются на дочернюю страницу Change history |

Видимые labels страницы подчиняются общему правилу двуязычности §4; машинные
идентификаторы не переводятся.

## Как устанавливаются шаблоны

Файлы `*-body.md` — локальные эталоны для ручной настройки
**Space Settings → Templates**. Они не публикуются как страницы и не являются
документами: это исходник шаблона пространства.

## Тела по типам документа

| Тип документа | Файл |
|---|---|
| Tutorial | [`tutorial-body.md`](tutorial-body.md) |
| How-to | [`how-to-body.md`](how-to-body.md) |
| Reference | [`reference-body.md`](reference-body.md) |
| Explanation | [`explanation-body.md`](explanation-body.md) |
| RFC | [`rfc-body.md`](rfc-body.md) |
| ADR | [`adr-body.md`](adr-body.md) |
| Incident | [`incident-body.md`](incident-body.md) |
| Problem Record | [`problem-record-body.md`](problem-record-body.md) |

Восемь из восьми типов §8 контракта покрыты.

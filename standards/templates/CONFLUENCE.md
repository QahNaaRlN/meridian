# Единый контракт шаблонов Confluence

Каноническое правило — §14 действующего Documentation Standard (точная ссылка и версия — `canonical_wiki.documentation_standard_url` / `documentation_standard_version` в `$MERIDIAN_INSTANCE/product.yaml`).

- Page Title — единственный H1. Тело страницы не содержит повторяющий `# Заголовок` и начинает содержательные разделы с H2.
- Первая конструкция тела — таблица `Поле (Field)` / `Значение (Value)`.
- Общий порядок: document type, status, primary semantic owner, scope, owner/author, created, updated, related documents, related tickets; затем специфичные поля.
- Видимые labels и контролируемые status values двуязычны: русский термин и английский идентификатор в скобках. Машинные identifiers не переводятся.
- Неприменимые условные строки удаляются. Каноническая Confluence-страница не хранит самоссылку `canonical_url`.
- Versioned standards ссылаются на дочернюю Change history; unversioned documents используют Updated и Confluence version history.

Файлы `*.confluence-template.md` ниже являются локальными эталонами для ручной настройки **Space Settings → Templates**.

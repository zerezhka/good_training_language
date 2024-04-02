# Хороший Учебный Язык (ХУЯ)

*Если Вам не нравится название языка, можете опустить букву Х, и называть его просто Учебным Языком. Но знайте — в таком случае язык уже не будет «Хорошим»! ;)*

Проект по большей части вдохновлён, но не основан на Учебном Алгоритмическом Языке Андрея Петровича Ершова. Рекомендую не воспринимать данный проект всерьёз, т.к. он готовился как шутка к Первому Апреля. Не смотря на свою шуточность, это более менее полноценный язык, на котором можно даже что-то писать (см. папку [примеры](./примеры/)).

Имейте ввиду, что т.к. у меня не было много времени, проект разрабатывался второпях, и содержит много багов и не законченых частей. Также заранее извеняюсь за отсутствие какой-либо нормальной документации. Если у меня будет время, я её добавлю в каком-нибудь виде. Наслаждайтесь!

## Быстрый Старт

Вам потребуется установить компилятор [Rust](https://www.rust-lang.org/) и [fasm](https://flatassembler.net/).

```console
$ rustc ./исходники/хуяк.rs
```

### Компиляция в Исполняемый Файл

На данный момент поддерживается только платформа Linux x86_64.

```console
$ ./хуяк комп ./примеры/01-привет.хуя
$ ./примеры/01-привет
```

Для других платформ можно попробовать Интерпретацию.

### Интерпретация

Интерпретация просто опускает стадию генерации машинного кода из промежуточного представления (ПП) и тупо интерпретирует ПП. По идее, это должно быть кроссплатформенным, но я не гарантирую, что не добавлю что-нибудь платформо-зависимое в ПП в будущем.

```console
$ ./хуяк интер ./примеры/01-привет.хуя
```

## Источники

- Wikipedia - Учебный алгоритмический язык - https://ru.wikipedia.org/wiki/Учебный_алгоритмический_язык (рус.) - проект по-большей части вдохновлён, но не основан на Учебном Алгоритмическом Языке Андрея Петровича Ершова.
- Tsoding - Porth - https://gitlab.com/tsoding/porth (анг.) - очень много идей реализации были разработаны и опробованны мною еще в Porth.
- flat assembler - https://flatassembler.net/ (анг.) - простой ассемблер способный генерировать минималистичные статические исполняемые файлы.
- Félix Cloutier - x86 and amd64 instruction reference - https://www.felixcloutier.com/x86/ (анг.) - онлайн справочник по инструкциям процессора Intel x86.
- ChromiumOS - Linux System Call Table - https://chromium.googlesource.com/chromiumos/docs/+/HEAD/constants/syscalls.md (анг.) - Таблица системных вызовов операционной системы Linux.
- compiler.su - Шестнадцатиричные и двоичные константы http://compiler.su/shestnadtsatirichnye-i-dvoichnye-konstanty.php (рус.) - идея шестнадцатиричных литералов была взята от сюда.

# Настройка кастомного домена ely.local

Чтобы использовать `ely.local` вместо `localhost` в адресной строке, нужно добавить запись в hosts файл.

## macOS / Linux

1. Открой терминал
2. Отредактируй `/etc/hosts`:
   ```bash
   sudo nano /etc/hosts
   ```
   
3. Добавь строку:
   ```
   127.0.0.1 ely.local
   ```
   
4. Сохрани (Ctrl+O, Enter, Ctrl+X)

5. Очисти DNS кэш:
   ```bash
   # macOS
   sudo dscacheutil -flushcache; sudo killall -HUP mDNSResponder
   
   # Linux
   sudo systemd-resolve --flush-caches
   ```

## Windows

1. Открой Notepad **от имени администратора**
2. Открой файл: `C:\Windows\System32\drivers\etc\hosts`
3. Добавь строку:
   ```
   127.0.0.1 ely.local
   ```
4. Сохрани

5. Очисти DNS кэш:
   ```cmd
   ipconfig /flushdns
   ```

## Проверка

После настройки проверь:
```bash
ping ely.local
```

Должен вернуть `127.0.0.1`.

Теперь вместо `http://localhost:17081/e/...` будет `http://ely.local:17081/e/...`

## Альтернатива (без hosts файла)

Если не хочешь редактировать hosts, расширение автоматически использует `localhost` как fallback.


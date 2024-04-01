use std::io;
use std::io::{Read, Write, BufRead};
use std::convert::TryInto;
use super::Результат;
use std::mem;
use компилятор::{ПП, ВидИнструкции, Инструкция, Имена};
use типизация::*;

// Разметка памяти
// |    второй стек    | инициализированные данные | неинициализированные данные |    куча?    |
// ^                   ^
// 0                   Начало стека и данных. Стек растет в сторону нуля.

pub const РАЗМЕР_СЛОВА: usize = mem::size_of::<u64>();

#[derive(Default)]
pub struct Машина<'ы> {
    индекс_инструкции: usize,  // аналог rip
    кадр: usize,               // аналог rbp (для первого стека)
    второй_стек: usize,        // аналог rsp
    кадр_второго_стека: usize, // аналог rbp (для второго стека)
    // В каком-то смысле, эти переменные выше являются регистрами
    // нашей виртуальной машины, не смотря на то, что машина-то
    // стековая.

    pub стек: Vec<usize>,
    начало_данных: usize,
    начало_второго_стека: usize,
    pub память: Vec<u8>,
    инструкции: &'ы [Инструкция],
}

macro_rules! ошибка_времени_исполнения {
    ($машина:expr, $($аргы:tt)*) => {{
        let индекс_инструкции = $машина.индекс_инструкции;
        if let Some(инструкция) = $машина.инструкции.get(индекс_инструкции) {
            let вид_инструкции = &инструкция.вид;
            let ::диагностика::Лок{путь_к_файлу, строка, столбец} = &инструкция.лок;
            eprint!("{путь_к_файлу}:{строка}:{столбец}: {вид_инструкции:?}: ", путь_к_файлу = путь_к_файлу.display());
        }
        eprint!("ОШИБКА ВРЕМЕНИ ИСПОЛНЕНИЯ: {индекс_инструкции}: ");
        eprintln!($($аргы)*);
    }};
}

impl<'ы> Машина<'ы> {
    pub fn новая(пп: &ПП, объём_второго_стека: usize) -> Машина {
        let начало_данных = объём_второго_стека;
        let второй_стек = объём_второго_стека;
        let кадр_второго_стека = объём_второго_стека;
        let начало_второго_стека = объём_второго_стека;
        let mut машина = Машина {
            индекс_инструкции: 0,
            кадр: 0,
            второй_стек,
            кадр_второго_стека,
            стек: Vec::new(),

            начало_данных,
            начало_второго_стека,

            память: vec![],
            инструкции: &пп.код,
        };

        // СДЕЛАТЬ: Ресайз вектора капец какой медленный. Возможно из-за
        // инициализации. Надо что-нибудь с этим сделать.
        машина.память.resize(машина.память.len() + объём_второго_стека, 0);
        машина.память.extend_from_slice(пп.иниц_данные.as_slice());
        машина.память.resize(машина.память.len() + пп.размер_неиниц_данных, 0);
        машина
    }

    fn протолкнуть_значение_нат(&mut self, значение: usize) -> Результат<()> {
        self.стек.push(значение);
        Ok(())
    }

    fn вытолкнуть_значение_нат(&mut self) -> Результат<usize> {
        if let Some(значение) = self.стек.pop()  {
            Ok(значение)
        } else {
            ошибка_времени_исполнения!(self, "Опустошение стека");
            Err(())
        }
    }

    fn протолкнуть_значение_вещ32(&mut self, значение: f32) -> Результат<()> {
        let mut bytes = [0u8; 8];
        bytes[0..4].copy_from_slice(&значение.to_le_bytes());
        self.протолкнуть_значение_нат(usize::from_le_bytes(bytes))
    }

    fn вытолкнуть_значение_вещ32(&mut self) -> Результат<f32> {
        Ok(f32::from_le_bytes(self.вытолкнуть_значение_нат()?.to_le_bytes()[0..4].try_into().unwrap()))
    }

    // СДЕЛАТЬ: вариант функции срез_памяти, который возвращает массив размера известного на этапе компиляции
    // Возможно через какие-нибудь дженерики. Такой вариант будет очень удобен для чтения примитивных типов
    // из памяти безо всяких этих try_into().unwrap() и прочей лабуды.
    pub fn срез_памяти(&mut self, адрес: usize, размер: usize) -> Результат<&mut [u8]> {
        let мин = self.второй_стек;
        let макс = self.память.len();

        if адрес < мин {
            ошибка_времени_исполнения!(self, "Попытка получить доступ к некорректнному диапазону памяти [{начало}..{конец}). Разрешенный диапазон [{мин}..{макс})", начало = адрес, конец = адрес+размер);
            return Err(())
        }

        if let Some(срез) = self.память.get_mut(адрес..адрес+размер) {
            Ok(срез)
        } else {
            ошибка_времени_исполнения!(self, "Попытка получить доступ к некорректнному диапазону памяти [{начало}..{конец}). Разрешенный диапазон [{мин}..{макс})", начало = адрес, конец = адрес+размер);
            Err(())
        }
    }

    fn количество_элементов_стека(&self) -> usize {
        self.стек.len()
    }

    fn проверить_арность_аргументов(&self, арность: usize) -> Результат<()> {
        let размер_стека = self.количество_элементов_стека();
        if размер_стека < арность {
            ошибка_времени_исполнения!(self, "Недостаточно аргументов для инструкции. Требуется как минимум {арность}, но всего в стеке аргументов находится {размер_стека}.");
            Err(())
        } else {
            Ok(())
        }
    }

    fn инструкция(&self) -> Результат<&Инструкция> {
        match self.инструкции.get(self.индекс_инструкции) {
            Some(инструкция) => Ok(инструкция),
            None => {
                ошибка_времени_исполнения!(self, "некорректный индекс инструкции");
                Err(())
            }
        }
    }

    fn выделить_на_втором_стеке(&mut self, размер: usize) -> Результат<()> {
        if self.второй_стек < размер {
            ошибка_времени_исполнения!(self, "переполнение второго стека");
            return Err(())
        }
        self.второй_стек -= размер;
        Ok(())
    }

    fn освободить_со_второго_стека(&mut self, размер: usize) -> Результат<()> {
        if размер > self.начало_второго_стека - self.второй_стек {
            ошибка_времени_исполнения!(self, "опустошение второго стека");
            return Err(())
        }
        self.второй_стек += размер;
        Ok(())
    }

    fn протолкнуть_на_второй_стек(&mut self, значение: usize) -> Результат<()> {
        self.выделить_на_втором_стеке(РАЗМЕР_СЛОВА)?;
        self.срез_памяти(self.второй_стек, РАЗМЕР_СЛОВА)?.copy_from_slice(&значение.to_le_bytes());
        Ok(())
    }

    fn вытолкнуть_из_второго_стека(&mut self) -> Результат<usize> {
        let значение = usize::from_le_bytes(self.срез_памяти(self.второй_стек, РАЗМЕР_СЛОВА)?.try_into().unwrap());
        self.освободить_со_второго_стека(РАЗМЕР_СЛОВА)?;
        Ok(значение)
    }

    pub fn интерпретировать(&mut self, имена: &Имена, точка_входа: usize, режим_отладки: bool) -> Результат<()> {
        self.индекс_инструкции = точка_входа;

        let mut глубина_вызовов = 0;
        let mut цель_перешагивания: Option<usize> = None;
        self.протолкнуть_значение_нат(self.инструкции.len())?;
        loop {
            let индекс_инструкции = self.индекс_инструкции;
            let инструкция = self.инструкция()?;

            if режим_отладки {
                if let Some(цель) = цель_перешагивания.clone() {
                    if глубина_вызовов <= цель {
                        цель_перешагивания = None;
                    }
                }

                if цель_перешагивания.is_none() {
                    диагностика!(&инструкция.лок, "ИНСТРУКЦИЯ", "{индекс_инструкции}: {вид_инструкции:?}", вид_инструкции = инструкция.вид);
                    eprintln!("стек = {стек:?}", стек = self.стек);
                    eprintln!("кадр = {кадр}", кадр = self.кадр);
                    eprintln!("второй_стек = {второй_стек}", второй_стек = self.второй_стек);
                    eprintln!("кадр_второго_стека = {кадр_второго_стека}", кадр_второго_стека = self.кадр_второго_стека);
                    eprintln!("переменные");
                    for (имя, переменная) in имена.переменные.iter() {
                        let адрес = self.начало_данных + переменная.смещение as usize;
                        eprintln!("  {имя}: {адрес:#X} = {:?}", &self.память[адрес..адрес+переменная.тип.размер(&имена.структуры)]);
                    }
                    loop {
                        let mut команда = String::new();
                        eprint!("> ");
                        io::stdin().lock().read_line(&mut команда).unwrap();
                        let аргы: Vec<&str> = команда.trim().split(' ').filter(|арг| арг.len() > 0).collect();
                        match аргы.as_slice() {
                            ["выход", ..] => {
                                return Ok(());
                            }
                            ["инст", парам @ ..] => match парам {
                                [инст] => match инст.parse::<usize>() {
                                    Ok(индекс_инструкции) => if let Some(инструкция) = self.инструкции.get(индекс_инструкции) {
                                        диагностика!(&инструкция.лок, "ИНСТРУКЦИЯ", "{индекс_инструкции}: {вид_инструкции:?}", вид_инструкции = инструкция.вид);
                                    } else {
                                        eprintln!("ОШИБКА: нету инструкции под номером {индекс_инструкции}")
                                    },
                                    Err(_ошибка) => {
                                        eprintln!("ОШИБКА: индекс инструкции не является корректным целым числом");
                                    },
                                },
                                _ => {
                                    eprintln!("Пример: инст [индекс_инструкции]");
                                    eprintln!("ОШИБКА: требуется индекс инструкции");
                                }
                            }
                            ["перешаг", ..] => {
                                цель_перешагивания = Some(глубина_вызовов);
                                break
                            }
                            [команда, ..] => {
                                eprintln!("ОШИБКА: неизвестная команда «{команда}»");
                            }
                            [] => {
                                break
                            }
                        }
                    }
                }
            }

            match &инструкция.вид {
                ВидИнструкции::Ноп => {
                    self.индекс_инструкции += 1;
                }
                ВидИнструкции::Вытолкнуть => {
                    self.вытолкнуть_значение_нат()?;
                    self.индекс_инструкции += 1;
                }
                ВидИнструкции::Продублировать => {
                    let значение = self.вытолкнуть_значение_нат()?;
                    self.протолкнуть_значение_нат(значение)?;
                    self.протолкнуть_значение_нат(значение)?;
                    self.индекс_инструкции += 1;
                }
                &ВидИнструкции::ГлобальныеДанные(смещение) => {
                    self.протолкнуть_значение_нат((self.начало_данных as i32 + смещение) as usize)?;
                    self.индекс_инструкции += 1;
                }
                &ВидИнструкции::Натуральное(значение)  => {
                    self.протолкнуть_значение_нат(значение)?;
                    self.индекс_инструкции += 1;
                }
                &ВидИнструкции::Целое(значение)  => {
                    self.протолкнуть_значение_нат(значение as usize)?;
                    self.индекс_инструкции += 1;
                }
                &ВидИнструкции::ВыделитьНаСтеке(размер) => {
                    self.выделить_на_втором_стеке(размер as usize)?;
                    self.индекс_инструкции += 1;
                }
                &ВидИнструкции::ОсвободитьСоСтека(размер) => {
                    self.освободить_со_второго_стека(размер as usize)?;
                    self.индекс_инструкции += 1;
                }
                &ВидИнструкции::ВершинаСтека(смещение) => {
                    self.протолкнуть_значение_нат((self.второй_стек as i32 + смещение) as usize)?;
                    self.индекс_инструкции += 1;
                }
                ВидИнструкции::СохранитьКадр => {
                    let старый_кадр = self.кадр_второго_стека;
                    self.кадр_второго_стека = self.второй_стек;
                    self.протолкнуть_на_второй_стек(старый_кадр)?;
                    self.индекс_инструкции += 1;
                }
                ВидИнструкции::ВосстановитьКадр => {
                    self.кадр_второго_стека = self.вытолкнуть_из_второго_стека()?;
                    self.индекс_инструкции += 1;
                }
                ВидИнструкции::Кадр(смещение) => {
                    self.протолкнуть_значение_нат((self.кадр_второго_стека as i32 + смещение) as usize)?;
                    self.индекс_инструкции += 1;
                }
                ВидИнструкции::АргументНаСтек => {
                    let значение = self.вытолкнуть_значение_нат()?;
                    self.протолкнуть_на_второй_стек(значение)?;
                    self.индекс_инструкции += 1;
                }
                ВидИнструкции::АргументСоСтека => {
                    let значение = self.вытолкнуть_из_второго_стека()?;
                    self.протолкнуть_значение_нат(значение)?;
                    self.индекс_инструкции += 1;
                }
                &ВидИнструкции::ВнутреннийВызов(адрекс) => {
                    глубина_вызовов += 1;
                    self.протолкнуть_значение_нат(индекс_инструкции + 1)?;
                    self.индекс_инструкции = адрекс;
                }
                ВидИнструкции::ВнешнийВызов{..} => {
                    ошибка_времени_исполнения!(self, "вынешние вызовы не поддерживаются в режиме интерпретации");
                    return Err(())
                }
                ВидИнструкции::Записать8 => {
                    self.проверить_арность_аргументов(2)?;
                    let адрес = self.вытолкнуть_значение_нат()?;
                    let значение = (self.вытолкнуть_значение_нат()? & 0xFF) as u8;
                    let тип = Тип::Нат8;
                    self.срез_памяти(адрес, тип.размер(&имена.структуры))?.copy_from_slice(&значение.to_le_bytes());
                    self.индекс_инструкции += 1;
                }
                ВидИнструкции::Записать16 => {
                    сделать!(&инструкция.лок, "Интерпретация инструкции Записать16");
                    return Err(());
                }
                ВидИнструкции::Записать32 => {
                    self.проверить_арность_аргументов(2)?;
                    let адрес = self.вытолкнуть_значение_нат()?;
                    let значение = self.вытолкнуть_значение_нат()? as u32;
                    self.срез_памяти(адрес, 4)?.copy_from_slice(&значение.to_le_bytes());
                    self.индекс_инструкции += 1;
                }
                ВидИнструкции::Записать64 => {
                    self.проверить_арность_аргументов(2)?;
                    let адрес = self.вытолкнуть_значение_нат()?;
                    let значение = self.вытолкнуть_значение_нат()?;
                    let тип = Тип::Нат64;
                    self.срез_памяти(адрес, тип.размер(&имена.структуры))?.copy_from_slice(&значение.to_le_bytes());
                    self.индекс_инструкции += 1;
                }
                ВидИнструкции::ПрочитатьБезЗнак8 => {
                    self.проверить_арность_аргументов(1)?;
                    let адрес = self.вытолкнуть_значение_нат()?;
                    let значение: u8 = u8::from_le_bytes(self.срез_памяти(адрес, 1)?.try_into().unwrap());
                    self.протолкнуть_значение_нат(значение as usize)?;
                    self.индекс_инструкции += 1;
                }
                ВидИнструкции::ПрочитатьБезЗнак16 => {
                    сделать!(&инструкция.лок, "Интерпртация инструкции ПрочитатьБезЗнак32");
                    return Err(());
                }
                ВидИнструкции::ПрочитатьБезЗнак32 => {
                    self.проверить_арность_аргументов(1)?;
                    let адрес = self.вытолкнуть_значение_нат()?;
                    let значение: u32 = u32::from_le_bytes(self.срез_памяти(адрес, 4)?.try_into().unwrap());
                    self.протолкнуть_значение_нат(значение as usize)?;
                    self.индекс_инструкции += 1;
                }
                ВидИнструкции::ПрочитатьЗнак8 => {
                    сделать!(&инструкция.лок, "Интерпртация инструкции ПрочитатьЗнак8");
                    return Err(());
                }
                ВидИнструкции::ПрочитатьЗнак16 => {
                    сделать!(&инструкция.лок, "Интерпртация инструкции ПрочитатьЗнак8");
                    return Err(());
                }
                ВидИнструкции::ПрочитатьЗнак32 => {
                    сделать!(&инструкция.лок, "Интерпртация инструкции ПрочитатьЗнак32");
                    return Err(());
                }
                ВидИнструкции::Прочитать64 => {
                    self.проверить_арность_аргументов(1)?;
                    let адрес = self.вытолкнуть_значение_нат()?;
                    let тип = Тип::Нат64;
                    let значение: u64 = u64::from_le_bytes(self.срез_памяти(адрес, тип.размер(&имена.структуры))?.try_into().unwrap());
                    self.протолкнуть_значение_нат(значение as usize)?;
                    self.индекс_инструкции += 1;
                }
                ВидИнструкции::СкопироватьПамять => {
                    let размер = self.вытолкнуть_значение_нат()?;
                    let цель = self.вытолкнуть_значение_нат()?;
                    let источник = self.вытолкнуть_значение_нат()?;
                    for индекс in 0..размер {
                        self.память[цель + индекс] = self.память[источник + индекс];
                    }
                    self.индекс_инструкции += 1;
                }
                ВидИнструкции::ПамятьРавно => {
                    let размер = self.вытолкнуть_значение_нат()?;
                    let цель = self.вытолкнуть_значение_нат()?;
                    let источник = self.вытолкнуть_значение_нат()?;
                    let mut равна = true;
                    for индекс in 0..размер {
                        if self.память[цель + индекс] != self.память[источник + индекс] {
                            равна = false;
                            break;
                        }
                    }
                    if равна {
                        self.протолкнуть_значение_нат(1)?;
                    } else {
                        self.протолкнуть_значение_нат(0)?;
                    }
                    self.индекс_инструкции += 1;
                }
                ВидИнструкции::НатМеньше => {
                    self.проверить_арность_аргументов(2)?;
                    let правый = self.вытолкнуть_значение_нат()?;
                    let левый = self.вытолкнуть_значение_нат()?;
                    if левый < правый {
                        self.протолкнуть_значение_нат(1)?;
                    } else {
                        self.протолкнуть_значение_нат(0)?;
                    }
                    self.индекс_инструкции += 1;
                }
                ВидИнструкции::НатМеньшеРавно => {
                    сделать!(&инструкция.лок, "Интерпретация инструкции «{вид:?}»", вид = инструкция.вид);
                    return Err(())
                }
                ВидИнструкции::НатБольше => {
                    self.проверить_арность_аргументов(2)?;
                    let правый = self.вытолкнуть_значение_нат()?;
                    let левый = self.вытолкнуть_значение_нат()?;
                    if левый > правый {
                        self.протолкнуть_значение_нат(1)?;
                    } else {
                        self.протолкнуть_значение_нат(0)?;
                    }
                    self.индекс_инструкции += 1;
                }
                ВидИнструкции::НатБольшеРавно => {
                    self.проверить_арность_аргументов(2)?;
                    let правый = self.вытолкнуть_значение_нат()?;
                    let левый = self.вытолкнуть_значение_нат()?;
                    if левый >= правый {
                        self.протолкнуть_значение_нат(1)?;
                    } else {
                        self.протолкнуть_значение_нат(0)?;
                    }
                    self.индекс_инструкции += 1;
                }
                ВидИнструкции::НатРавно => {
                    self.проверить_арность_аргументов(2)?;
                    let правый = self.вытолкнуть_значение_нат()?;
                    let левый = self.вытолкнуть_значение_нат()?;
                    if левый == правый {
                        self.протолкнуть_значение_нат(1)?;
                    } else {
                        self.протолкнуть_значение_нат(0)?;
                    }
                    self.индекс_инструкции += 1;
                }
                ВидИнструкции::НатСложение => {
                    self.проверить_арность_аргументов(2)?;
                    let правый = self.вытолкнуть_значение_нат()?;
                    let левый = self.вытолкнуть_значение_нат()?;
                    let (результат, _насрать) = левый.overflowing_add(правый);
                    self.протолкнуть_значение_нат(результат)?;
                    self.индекс_инструкции += 1;
                }
                ВидИнструкции::НатВычитание => {
                    self.проверить_арность_аргументов(2)?;
                    let правый = self.вытолкнуть_значение_нат()?;
                    let левый = self.вытолкнуть_значение_нат()?;
                    let (результат, _насрать) = левый.overflowing_sub(правый);
                    self.протолкнуть_значение_нат(результат)?;
                    self.индекс_инструкции += 1;
                }
                ВидИнструкции::НатУмножение => {
                    self.проверить_арность_аргументов(2)?;
                    let правый = self.вытолкнуть_значение_нат()?;
                    let левый = self.вытолкнуть_значение_нат()?;
                    self.протолкнуть_значение_нат(левый * правый)?;
                    self.индекс_инструкции += 1;
                }
                ВидИнструкции::НатДеление => {
                    self.проверить_арность_аргументов(2)?;
                    let правый = self.вытолкнуть_значение_нат()?;
                    let левый = self.вытолкнуть_значение_нат()?;
                    self.протолкнуть_значение_нат(левый / правый)?;
                    self.индекс_инструкции += 1;
                }
                ВидИнструкции::НатОстаток => {
                    self.проверить_арность_аргументов(2)?;
                    let правый = self.вытолкнуть_значение_нат()?;
                    let левый = self.вытолкнуть_значение_нат()?;
                    self.протолкнуть_значение_нат(левый % правый)?;
                    self.индекс_инструкции += 1;
                }
                ВидИнструкции::ЦелМеньше => {
                    self.проверить_арность_аргументов(2)?;
                    let правый = self.вытолкнуть_значение_нат()? as i64;
                    let левый = self.вытолкнуть_значение_нат()? as i64;
                    if левый < правый {
                        self.протолкнуть_значение_нат(1)?;
                    } else {
                        self.протолкнуть_значение_нат(0)?;
                    }
                    self.индекс_инструкции += 1;
                }
                ВидИнструкции::ЦелМеньшеРавно => {
                    self.проверить_арность_аргументов(2)?;
                    let правый = self.вытолкнуть_значение_нат()? as i64;
                    let левый = self.вытолкнуть_значение_нат()? as i64;
                    if левый <= правый {
                        self.протолкнуть_значение_нат(1)?;
                    } else {
                        self.протолкнуть_значение_нат(0)?;
                    }
                    self.индекс_инструкции += 1;
                }
                ВидИнструкции::ЦелБольше => {
                    self.проверить_арность_аргументов(2)?;
                    let правый = self.вытолкнуть_значение_нат()? as i64;
                    let левый = self.вытолкнуть_значение_нат()? as i64;
                    if левый > правый {
                        self.протолкнуть_значение_нат(1)?;
                    } else {
                        self.протолкнуть_значение_нат(0)?;
                    }
                    self.индекс_инструкции += 1;
                }
                ВидИнструкции::ЦелБольшеРавно => {
                    self.проверить_арность_аргументов(2)?;
                    let правый = self.вытолкнуть_значение_нат()? as i64;
                    let левый = self.вытолкнуть_значение_нат()? as i64;
                    if левый >= правый {
                        self.протолкнуть_значение_нат(1)?;
                    } else {
                        self.протолкнуть_значение_нат(0)?;
                    }
                    self.индекс_инструкции += 1;
                }
                ВидИнструкции::ЦелОтриц => {
                    self.проверить_арность_аргументов(1)?;
                    let значение = self.вытолкнуть_значение_нат()? as i64;
                    self.протолкнуть_значение_нат((-значение) as usize)?;
                    self.индекс_инструкции += 1;
                }
                ВидИнструкции::ЦелУмножение => {
                    self.проверить_арность_аргументов(2)?;
                    let правый = self.вытолкнуть_значение_нат()? as i64;
                    let левый = self.вытолкнуть_значение_нат()? as i64;
                    self.протолкнуть_значение_нат((левый * правый) as usize)?;
                    self.индекс_инструкции += 1;
                }
                ВидИнструкции::ЦелОстаток => {
                    self.проверить_арность_аргументов(2)?;
                    let правый = self.вытолкнуть_значение_нат()? as i64;
                    let левый = self.вытолкнуть_значение_нат()? as i64;
                    self.протолкнуть_значение_нат((левый % правый) as usize)?;
                    self.индекс_инструкции += 1;
                }
                ВидИнструкции::КонвертНат64Вещ32 => {
                    let значение = self.вытолкнуть_значение_нат()?;
                    self.протолкнуть_значение_вещ32(значение as f32)?;
                    self.индекс_инструкции += 1;
                }
                ВидИнструкции::КонвертЦел64Вещ32 => {
                    let значение = self.вытолкнуть_значение_нат()? as i64;
                    self.протолкнуть_значение_вещ32(значение as f32)?;
                    self.индекс_инструкции += 1;
                }
                ВидИнструкции::КонвертВещ32Нат64 => {
                    let значение = self.вытолкнуть_значение_вещ32()?;
                    self.протолкнуть_значение_нат(значение as usize)?;
                    self.индекс_инструкции += 1;
                }
                ВидИнструкции::КонвертВещ32Цел64 => {
                    let значение = self.вытолкнуть_значение_вещ32()?;
                    self.протолкнуть_значение_нат((значение as i64) as usize)?;
                    self.индекс_инструкции += 1;
                }
                ВидИнструкции::Вещ32Умножение => {
                    self.проверить_арность_аргументов(2)?;
                    let правый = self.вытолкнуть_значение_вещ32()?;
                    let левый = self.вытолкнуть_значение_вещ32()?;
                    self.протолкнуть_значение_вещ32(левый * правый)?;
                    self.индекс_инструкции += 1;
                }
                ВидИнструкции::Вещ32Деление => {
                    сделать!(&инструкция.лок, "Интерпретация инструкции Вещ32Деление");
                    return Err(());
                }
                ВидИнструкции::Вещ32Сложение => {
                    self.проверить_арность_аргументов(2)?;
                    let правый = self.вытолкнуть_значение_вещ32()?;
                    let левый = self.вытолкнуть_значение_вещ32()?;
                    self.протолкнуть_значение_вещ32(левый + правый)?;
                    self.индекс_инструкции += 1;
                }
                ВидИнструкции::Вещ32Меньше => {
                    self.проверить_арность_аргументов(2)?;
                    let правый = self.вытолкнуть_значение_вещ32()?;
                    let левый = self.вытолкнуть_значение_вещ32()?;
                    if левый < правый {
                        self.протолкнуть_значение_нат(1)?;
                    } else {
                        self.протолкнуть_значение_нат(0)?;
                    }
                    self.индекс_инструкции += 1;
                }
                ВидИнструкции::Вещ32МеньшеРавно => {
                    сделать!(&инструкция.лок, "Интерпретация инструкции «{вид:?}»", вид = инструкция.вид);
                    return Err(())
                }
                ВидИнструкции::Вещ32Больше => {
                    self.проверить_арность_аргументов(2)?;
                    let правый = self.вытолкнуть_значение_вещ32()?;
                    let левый = self.вытолкнуть_значение_вещ32()?;
                    if левый > правый {
                        self.протолкнуть_значение_нат(1)?;
                    } else {
                        self.протолкнуть_значение_нат(0)?;
                    }
                    self.индекс_инструкции += 1;
                }
                ВидИнструкции::Вещ32БольшеРавно => {
                    self.проверить_арность_аргументов(2)?;
                    let правый = self.вытолкнуть_значение_вещ32()?;
                    let левый = self.вытолкнуть_значение_вещ32()?;
                    if левый >= правый {
                        self.протолкнуть_значение_нат(1)?;
                    } else {
                        self.протолкнуть_значение_нат(0)?;
                    }
                    self.индекс_инструкции += 1;
                }
                ВидИнструкции::Вещ32Отриц => {
                    let значение = self.вытолкнуть_значение_вещ32()?;
                    self.протолкнуть_значение_вещ32(-значение)?;
                    self.индекс_инструкции += 1;
                }
                ВидИнструкции::ЛогОтрицание => {
                    self.проверить_арность_аргументов(1)?;
                    let значение = self.вытолкнуть_значение_нат()?;
                    if значение == 0 {
                        self.протолкнуть_значение_нат(1)?;
                    } else {
                        self.протолкнуть_значение_нат(0)?;
                    }
                    self.индекс_инструкции += 1;
                }
                ВидИнструкции::ЛогИ => {
                    let а = self.вытолкнуть_значение_нат()? != 0;
                    let б = self.вытолкнуть_значение_нат()? != 0;
                    self.протолкнуть_значение_нат((а && б) as usize)?;
                    self.индекс_инструкции += 1;
                }
                ВидИнструкции::ЛогЛибо => {
                    let а = self.вытолкнуть_значение_нат()? != 0;
                    let б = self.вытолкнуть_значение_нат()? != 0;
                    self.протолкнуть_значение_нат((а != б) as usize)?;
                    self.индекс_инструкции += 1;
                }
                ВидИнструкции::БитИли => {
                    let а = self.вытолкнуть_значение_нат()?;
                    let б = self.вытолкнуть_значение_нат()?;
                    self.протолкнуть_значение_нат(а | б)?;
                    self.индекс_инструкции += 1;
                }
                ВидИнструкции::БитИ => {
                    let а = self.вытолкнуть_значение_нат()?;
                    let б = self.вытолкнуть_значение_нат()?;
                    self.протолкнуть_значение_нат(а & б)?;
                    self.индекс_инструкции += 1;
                }
                ВидИнструкции::БитЛибо => {
                    let а = self.вытолкнуть_значение_нат()?;
                    let б = self.вытолкнуть_значение_нат()?;
                    self.протолкнуть_значение_нат(а ^ б)?;
                    self.индекс_инструкции += 1;
                }
                ВидИнструкции::БитСмещениеВлево => {
                    let сдвиг = self.вытолкнуть_значение_нат()?;
                    let значение = self.вытолкнуть_значение_нат()?;
                    self.протолкнуть_значение_нат(значение << сдвиг)?;
                    self.индекс_инструкции += 1;
                }
                ВидИнструкции::БитСмещениеВправо => {
                    let сдвиг = self.вытолкнуть_значение_нат()?;
                    let значение = self.вытолкнуть_значение_нат()?;
                    self.протолкнуть_значение_нат(значение >> сдвиг)?;
                    self.индекс_инструкции += 1;
                }
                ВидИнструкции::Прыжок(индекс) => {
                    self.индекс_инструкции = *индекс;
                }
                &ВидИнструкции::УсловныйПрыжок(индекс) => {
                    self.проверить_арность_аргументов(1)?;
                    let значение = self.вытолкнуть_значение_нат()?;
                    if значение == 0 {
                        self.индекс_инструкции += 1;
                    } else {
                        self.индекс_инструкции = индекс;
                    }
                }
                ВидИнструкции::ПечатьСтроки => {
                    self.проверить_арность_аргументов(1)?;
                    let строка = self.вытолкнуть_значение_нат()?;
                    let адрес = usize::from_le_bytes(
                        self.срез_памяти(строка + СРЕЗ_АДРЕС_СМЕЩЕНИЕ, РАЗМЕР_СЛОВА)?.try_into().unwrap()
                    );
                    let размер = usize::from_le_bytes(
                        self.срез_памяти(строка + СРЕЗ_РАЗМЕР_СМЕЩЕНИЕ, РАЗМЕР_СЛОВА)?.try_into().unwrap()
                    );
                    let _ = io::stdout().write(self.срез_памяти(адрес, размер)?);
                    let _ = io::stdout().flush();
                    self.индекс_инструкции += 1;
                }
                ВидИнструкции::Ввод => {
                    self.проверить_арность_аргументов(2)?;
                    let длинна = self.вытолкнуть_значение_нат()?;
                    let указатель = self.вытолкнуть_значение_нат()?;
                    let размер = io::stdin().read(self.срез_памяти(указатель, длинна)?).unwrap();
                    self.протолкнуть_значение_нат(размер)?;
                    self.индекс_инструкции += 1;
                }
                ВидИнструкции::Возврат => {
                    // СДЕЛАТЬ: Ввести отдельную инструкцию останова.
                    // И генерировать точку входа наподобии того, как мы это делаем в эльф.
                    // Т.е. точка входа 0. Он прыгает в главную, и после вызывает останов.
                    if глубина_вызовов == 0 {
                        break;
                    }
                    self.индекс_инструкции = self.вытолкнуть_значение_нат()?;
                    глубина_вызовов -= 1;
                },
                ВидИнструкции::СисВызов {..} => {
                    ошибка_времени_исполнения!(self, "системные вызовы не поддерживаются в режиме интерпретации");
                    return Err(())
                }
            }
        }
        Ok(())
    }
}

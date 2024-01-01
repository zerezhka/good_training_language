/// Промежуточное Представление

use super::Результат;
use std::collections::HashMap;
use синтаксис::*;
use диагностика::*;
use лексика::*;

/// Инструкция промежуточного представления
#[derive(Debug)]
pub enum Инструкция {
    Ноп,
    /// Протолкнуть целое значение на стек аргументов.
    ПротолкнутьЦелое(usize),
    /// Протолкнуть указатель на инициализированные данные.
    ///
    /// Эта инстуркция нужна потому, что мы не знаем во время
    /// компиляции где начинаются данные. Мы это только знаем во время
    /// интерпретации, либо генерации машинного кода.
    ПротолкнутьИницУказатель(usize), // СДЕЛАТЬ: по возможности, использовать u64 вместо usize для значений пп
    /// Протолкнуть указатель на неинициализированные данные.
    ПротолкнутьНеиницУказатель(usize),
    ОпределитьЛокальный,
    СброситьЛокальный,
    ВызватьПроцедуру(usize),
    Записать64,
    Прочитать64,
    ЦелСложение,
    ЦелМеньше,
    ЛогОтрицание,
    ПечатьСтроки,
    ПечатьЦелого,
    ПечатьЛогического,
    Возврат,
    Прыжок(usize),
    УсловныйПрыжок(usize),
}

#[derive(Clone)]
pub struct СкомпПеременная {
    pub синтаксис: Переменная,
    pub адрес: usize,
}

#[derive(Debug)]
pub struct СкомпПроцедура {
    pub синтаксис: Процедура,
    pub точка_входа: usize,
}

#[derive(Debug)]
pub struct СкомпКонстанта {
    pub синтаксис: Константа,
    pub значение: usize,
}

/// Промежуточное Представление
#[derive(Default)]
pub struct ПП {
    pub код: Vec<Инструкция>,
    pub иниц_данные: Vec<u8>,
    pub размер_неиниц_данных: usize,
}

impl ПП {
    pub fn вывалить(&self) {
        println!("Инструкции ({количество} {инструкций}):",
                 количество = self.код.len(),
                 инструкций = ЧИСУЩ_ИНСТРУКЦИЙ.текст(self.код.len()));
        let ширина_столбца_индекса = self.код.len().to_string().len();
        for (индекс, инструкция) in self.код.iter().enumerate() {
            println!("{индекс:0>ширина_столбца_индекса$}: {инструкция:?}")
        }
        println!();
        println!("Инициализированные данные ({размер} {байт}):",
                 размер = self.иниц_данные.len(),
                 байт = ЧИСУЩ_БАЙТ.текст(self.иниц_данные.len()));
        const ШИРИНА_КОЛОНКИ_ИНИЦ_ДАННЫХ: usize = 16;
        for строка in 0..self.иниц_данные.len()/ШИРИНА_КОЛОНКИ_ИНИЦ_ДАННЫХ {
            let адрес = строка*ШИРИНА_КОЛОНКИ_ИНИЦ_ДАННЫХ;
            print!("{адрес:#08X}: ");
            let байты = &self.иниц_данные[адрес..адрес + ШИРИНА_КОЛОНКИ_ИНИЦ_ДАННЫХ];
            for байт in байты {
                print!("{байт:#04X} ");
            }
            println!()
        }
        let остаток = self.иниц_данные.len()%ШИРИНА_КОЛОНКИ_ИНИЦ_ДАННЫХ;
        if остаток > 0 {
            let адрес = self.иниц_данные.len()/ШИРИНА_КОЛОНКИ_ИНИЦ_ДАННЫХ*ШИРИНА_КОЛОНКИ_ИНИЦ_ДАННЫХ;
            print!("{адрес:#08X}: ");
            let байты = &self.иниц_данные[адрес..адрес + остаток];
            for байт in байты {
                print!("{байт:#04X} ");
            }
            println!()
        }
        println!();
        println!("Размер неинициализированных данных: {размер} {байт}",
                 размер = self.размер_неиниц_данных,
                 байт = ЧИСУЩ_БАЙТ.текст(self.размер_неиниц_данных));
    }
}

#[derive(Default)]
pub struct Имена {
    pub константы: HashMap<String, СкомпКонстанта>,
    pub процедуры: HashMap<String, СкомпПроцедура>,
    pub переменные: HashMap<String, СкомпПеременная>,
}

impl Имена {
    fn верифицировать_переопределение_имени(&self, имя: &Лексема) -> Результат<()> {
        if let Some(существующая_переменная) = self.переменные.get(&имя.текст) {
            диагностика!(&имя.лок, "ОШИБКА",
                         "уже существует переменная с именем «{имя}»",
                         имя = имя.текст);
            диагностика!(&существующая_переменная.синтаксис.имя.лок, "ИНФО",
                         "она определена здесь здесь. Выберите другое имя.");
            return Err(())
        }

        if let Some(существующая_процедура) = self.процедуры.get(&имя.текст) {
            диагностика!(&имя.лок, "ОШИБКА",
                         "уже существует процедура с именем «{имя}»",
                         имя = имя.текст);
            диагностика!(&существующая_процедура.синтаксис.имя.лок, "ИНФО",
                         "она определена здесь здесь. Выберите другое имя.");
            return Err(())
        }

        if let Some(существующая_константа) = self.константы.get(&имя.текст) {
            диагностика!(&имя.лок, "ОШИБКА",
                         "уже существует константа с именем «{имя}»",
                         имя = имя.текст);
            диагностика!(&существующая_константа.синтаксис.имя.лок, "ИНФО",
                         "она определена здесь здесь. Выберите другое имя.");
            return Err(())
        }

        Ok(())
    }
}

#[derive(Default)]
pub struct Программа {
    pub пп: ПП,
    pub имена: Имена,
}

fn скомпилировать_выражение(пп: &mut ПП, имена: &Имена, выражение: &Выражение) -> Результат<Тип> {
    match выражение {
        Выражение::Число(_, число) => {
            пп.код.push(Инструкция::ПротолкнутьЦелое(*число));
            Ok(Тип::Цел)
        },
        Выражение::Строка(строка) => {
            let указатель = пп.иниц_данные.len();
            let длинна = строка.текст.len();
            пп.иниц_данные.extend(строка.текст.as_bytes());
            пп.код.push(Инструкция::ПротолкнутьЦелое(длинна));
            пп.код.push(Инструкция::ПротолкнутьИницУказатель(указатель));
            Ok(Тип::Строка)
        }
        Выражение::Идент(лексема) => {
            if let Some(константа) = имена.константы.get(&лексема.текст) {
                пп.код.push(Инструкция::ПротолкнутьЦелое(константа.значение));
                return Ok(Тип::Цел);
            }
            if let Some(переменная) = имена.переменные.get(&лексема.текст) {
                пп.код.push(Инструкция::ПротолкнутьНеиницУказатель(переменная.адрес));
                match переменная.синтаксис.тип {
                    Тип::Цел => {
                        пп.код.push(Инструкция::Прочитать64);
                        return Ok(Тип::Цел);
                    }
                    Тип::Лог => {
                        сделать!(&лексема.лок, "чтение логических переменных");
                        return Err(())
                    }
                    Тип::Строка => {
                        сделать!(&лексема.лок, "чтение строковых переменных");
                        return Err(())
                    }
                }
            }
            диагностика!(&лексема.лок, "ОШИБКА",
                         "не существует ни констант, ни переменных с имением «{имя}»",
                         имя = &лексема.текст);
            Err(())
        }
        Выражение::Биноп {ключ: _, вид, левое, правое} => {
            let левый_тип = скомпилировать_выражение(пп, имена, &левое)?;
            let правый_тип = скомпилировать_выражение(пп, имена, &правое)?;
            match вид {
                ВидБинопа::Меньше => {
                    проверить_типы(левое.лок(), &Тип::Цел, &левый_тип)?;
                    проверить_типы(правое.лок(), &Тип::Цел, &правый_тип)?;
                    пп.код.push(Инструкция::ЦелМеньше);
                    Ok(Тип::Лог)
                }
                ВидБинопа::Сложение => {
                    проверить_типы(левое.лок(), &Тип::Цел, &левый_тип)?;
                    проверить_типы(правое.лок(), &Тип::Цел, &правый_тип)?;
                    пп.код.push(Инструкция::ЦелСложение);
                    Ok(Тип::Цел)
                }
            }
        }
    }
}

fn скомпилировать_утвержление(пп: &mut ПП, имена: &Имена, утверждение: &Утверждение) -> Результат<()> {
    match утверждение {
        Утверждение::Присваивание{имя, значение, ..} => {
            if let Some(переменная) = имена.переменные.get(имя.текст.as_str()) {
                let тип = скомпилировать_выражение(пп, имена, &значение)?;
                проверить_типы(&значение.лок(), &переменная.синтаксис.тип, &тип)?;
                пп.код.push(Инструкция::ПротолкнутьНеиницУказатель(переменная.адрес));
                пп.код.push(Инструкция::Записать64);
                Ok(())
            } else {
                диагностика!(&имя.лок, "ОШИБКА", "Неизвестная переменная «{имя}»", имя = имя.текст);
                return Err(())
            }
        },
        Утверждение::Вызов{имя, аргументы} => {
            match имя.текст.as_str() {
                // СДЕЛАТЬ: не позволять переопределять процедуру «печать» в пользовательском коде.
                "печать" => {
                    for арг in аргументы {
                        let тип = скомпилировать_выражение(пп, имена, &арг)?;
                        match тип {
                            Тип::Строка => пп.код.push(Инструкция::ПечатьСтроки),
                            Тип::Цел => пп.код.push(Инструкция::ПечатьЦелого),
                            Тип::Лог => пп.код.push(Инструкция::ПечатьЛогического),
                        }
                    }
                    Ok(())
                },
                _ => {
                    if let Some(процедура) = имена.процедуры.get(&имя.текст) {
                        let количество_аргументов = аргументы.len();
                        let количество_параметров = процедура.синтаксис.параметры.len();
                        if количество_аргументов != количество_параметров {
                            диагностика!(&имя.лок, "ОШИБКА",
                                         "Неверное количество аргументов вызова процедуры. Процедура принимает {количество_параметров} {параметров}, но в данном вызове предоставлено лишь {количество_аргументов} {аргументов}.",
                                         параметров = ЧИСУЩ_ПАРАМЕТР.текст(количество_параметров),
                                         аргументов = ЧИСУЩ_АРГУМЕНТ.текст(количество_аргументов));
                            return Err(());
                        }

                        for (параметр, аргумент) in процедура.синтаксис.параметры.iter().zip(аргументы.iter()) {
                            let тип = скомпилировать_выражение(пп, имена, аргумент)?;
                            проверить_типы(&аргумент.лок(), &параметр.тип, &тип)?;
                            if параметр.тип == Тип::Цел {
                                пп.код.push(Инструкция::ОпределитьЛокальный);
                            } else {
                                сделать!(&параметр.имя.лок, "Определение локальных переменных типа «{тип:?}»", тип = параметр.тип);
                                return Err(())
                            }
                        }
                        пп.код.push(Инструкция::ВызватьПроцедуру(процедура.точка_входа));
                        Ok(())
                    } else {
                        диагностика!(&имя.лок, "ОШИБКА", "Неизвестная процедура «{имя}»", имя = имя.текст);
                        Err(())
                    }
                }
            }
        }
        Утверждение::Пока{ключ: _, условие, тело} => {
            let точка_условия = пп.код.len();
            let тип = скомпилировать_выражение(пп, имена, &условие)?;
            проверить_типы(&условие.лок(), &Тип::Лог, &тип)?;
            пп.код.push(Инструкция::ЛогОтрицание);
            let точка_условного_прыжка = пп.код.len();
            пп.код.push(Инструкция::Ноп);
            for утверждение in тело.iter() {
                скомпилировать_утвержление(пп, имена, утверждение)?;
            }
            пп.код.push(Инструкция::Прыжок(точка_условия));
            let точка_выхода = пп.код.len();
            пп.код[точка_условного_прыжка] = Инструкция::УсловныйПрыжок(точка_выхода);
            Ok(())
        }
    }
}

fn скомпилировать_процедуру(пп: &mut ПП, имена: &Имена, процедура: Процедура) -> Результат<СкомпПроцедура> {
    let точка_входа = пп.код.len();
    for утверждение in &процедура.тело {
        скомпилировать_утвержление(пп, имена, утверждение)?;
    }
    for параметр in &процедура.параметры {
        if параметр.тип == Тип::Цел {
            пп.код.push(Инструкция::СброситьЛокальный);
        } else {
            сделать!(&параметр.имя.лок, "Сброс локальных переменных типа «{тип:?}»", тип = параметр.тип);
            return Err(())
        }
    }
    пп.код.push(Инструкция::Возврат);
    Ok(СкомпПроцедура{синтаксис: процедура, точка_входа})
}

fn интерпретировать_выражение_константы(константы: &HashMap<String, СкомпКонстанта>, выражение: &Выражение) -> Результат<usize> {
    match выражение {
        &Выражение::Число(_, число) => Ok(число),
        Выражение::Строка(строка) => {
            сделать!(&строка.лок, "строковые константы");
            Err(())
        }
        Выражение::Идент(имя) => {
            if let Some(константа) = константы.get(имя.текст.as_str()) {
                Ok(константа.значение)
            } else {
                диагностика!(&имя.лок, "ОШИБКА", "Неизвестная константа «{имя}»", имя = имя.текст);
                Err(())
            }
        }
        Выражение::Биноп{ключ, вид, левое, правое, ..} => {
            let левое_значение = интерпретировать_выражение_константы(константы, левое)?;
            let правое_значение = интерпретировать_выражение_константы(константы, правое)?;
            match вид {
                ВидБинопа::Меньше => {
                    сделать!(&ключ.лок, "булевые константы");
                    Err(())
                },
                ВидБинопа::Сложение => {
                    Ok(левое_значение + правое_значение)
                }
            }
        }
    }
}

impl Программа {
    pub fn скомпилировать_лексемы(&mut self, лекс: &mut Лексер) -> Результат<()> {
        loop {
            let ключ = лекс.вытащить_лексему_вида(&[
                ВидЛексемы::КлючПер,
                ВидЛексемы::КлючПро,
                ВидЛексемы::КлючКонст,
                ВидЛексемы::Конец,
            ])?;
            match ключ.вид {
                ВидЛексемы::КлючПер => {
                    let синтаксис = Переменная::разобрать(лекс)?;
                    self.имена.верифицировать_переопределение_имени(&синтаксис.имя)?;
                    let адрес = self.пп.размер_неиниц_данных;
                    self.пп.размер_неиниц_данных += синтаксис.тип.размер();
                    if let Some(_) = self.имена.переменные.insert(синтаксис.имя.текст.clone(), СкомпПеременная {адрес, синтаксис}) {
                        unreachable!("Проверка переопределения переменных должна происходить на этапе разбора")
                    }
                }
                ВидЛексемы::КлючПро => {
                    let процедура = Процедура::разобрать(лекс)?;
                    self.имена.верифицировать_переопределение_имени(&процедура.имя)?;
                    let скомп_процедура = скомпилировать_процедуру(&mut self.пп, &self.имена, процедура)?;
                    if let Some(_) = self.имена.процедуры.insert(скомп_процедура.синтаксис.имя.текст.clone(), скомп_процедура) {
                        unreachable!("Проверка переопределения процедур должна происходить на этапе разбора")
                    }
                }
                ВидЛексемы::КлючКонст => {
                    let константа = Константа::разобрать(лекс)?;
                    self.имена.верифицировать_переопределение_имени(&константа.имя)?;
                    let значение = интерпретировать_выражение_константы(&self.имена.константы, &константа.выражение)?;
                    if let Some(_) = self.имена.константы.insert(константа.имя.текст.clone(), СкомпКонстанта { синтаксис: константа, значение }) {
                        unreachable!("Проверка переопределения констант должна происходить на этапе разбора")
                    }
                }
                ВидЛексемы::Конец => return Ok(()),
                _ => unreachable!(),
            }
        }
    }
}

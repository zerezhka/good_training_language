use std::fs;
use std::env;
use std::io;
use std::process::{Command, ExitCode, Stdio};
use std::path::{Path, PathBuf};

#[path="./диагностика.rs"]
#[macro_use]
mod диагностика;
#[path="./лексика.rs"]
mod лексика;
#[path="./синтаксис.rs"]
mod синтаксис;
#[path="./компилятор.rs"]
mod компилятор;
#[path="./интерпретатор.rs"]
mod интерпретатор;
#[path="./типизация.rs"]
mod типизация;
#[path="./фазм.rs"]
mod фазм;

use диагностика::Лок;
use лексика::Лексер;
use компилятор::{Программа, ТочкаВхода};
use интерпретатор::Машина;

type Результат<Тэ> = Result<Тэ, ()>;

fn прочитать_содержимое_файла(путь_к_файлу: &Path, лок: Option<Лок>) -> Результат<Vec<char>> {
    fs::read_to_string(путь_к_файлу)
        .map(|содержимое| содержимое.chars().collect())
        .map_err(|ошибка| {
            // СДЕЛАТЬ: локализировать те ошибки, которые возможно.
            match ошибка.kind() {
                io::ErrorKind::NotFound => match лок {
                    Some(лок) => диагностика!(лок, "ОШИБКА", "файл «{путь_к_файлу}» не найден", путь_к_файлу = путь_к_файлу.display()),
                    None => eprintln!("ОШИБКА: файл «{путь_к_файлу}» не найден", путь_к_файлу = путь_к_файлу.display()),
                }
                _ => match лок {
                    Some(лок) => диагностика!(лок, "ОШИБКА", "не получилось прочитать файл «{путь_к_файлу}»: {ошибка}", путь_к_файлу = путь_к_файлу.display()),
                    None => eprintln!("ОШИБКА: не получилось прочитать файл «{путь_к_файлу}»: {ошибка}", путь_к_файлу = путь_к_файлу.display()),
                }
            }
        })
}

struct Команда {
    имя: &'static str,
    сигнатура: &'static str,
    описание: &'static str,
    запустить: fn(программа: &str, аргы: env::Args) -> Результат<()>,
}

// СДЕЛАТЬ: может имеет смысл реализовать генерацию бинарников из
// ассемблера ПП? Будет очень удобно отлаживать генерацию машинного
// кода, когда должный синтаксис еще не реализован.

const КОМАНДЫ: &[Команда] = &[
    Команда {
        имя: "комп",
        сигнатура: "[-пуск] [-вывод <файл-вывода>] <файл-ввода>",
        описание: "Скомпилировать файлы исходного кода в исполняемый файл для платформы Linux x86_64.",
        запустить: |программа, mut аргы| {
            let mut пуск = false;
            let mut фасм = false;
            let mut файл_ввода = None;
            let mut файл_вывода = None;

            loop {
                match аргы.next() {
                    Some(арг) => match арг.as_str() {
                        "-пуск" => пуск = true,
                        "-вывод" => {
                            match аргы.next() {
                                Some(арг) => файл_вывода = Some(арг),
                                None => {
                                    eprintln!("ОШИБКА: Флаг «{арг}» требует значение.");
                                    return Err(())
                                }
                            }
                        }
                        "-фасм" => фасм = true,
                        _ => {
                            if файл_ввода.is_some() {
                                пример(программа);
                                eprintln!("ОШИБКА: Неизвестный флаг «{арг}».");
                                return Err(())
                            } else {
                                файл_ввода = Some(арг)
                            }
                        }
                    }
                    None => break,
                }
            }

            let файл_ввода = if let Some(файл_ввода) = файл_ввода {
                PathBuf::from(файл_ввода)
            } else {
                пример(программа);
                eprintln!("ОШИБКА: требуется файл с программой!");
                return Err(());
            };

            let mut программа = Программа::default();
            let содержимое: Vec<char> = прочитать_содержимое_файла(&файл_ввода, None)?;
            let mut лекс = Лексер::новый(&файл_ввода, &содержимое);
            программа.скомпилировать_лексемы(&mut лекс)?;
            программа.завершить_компиляцию();
            let процедура_точки_входа = "главная";
            if let Some(процедура) = программа.имена.процедуры.get(процедура_точки_входа) {
                let точка_входа = match процедура.точка_входа {
                    ТочкаВхода::Внутреняя{адрес} => адрес,
                    ТочкаВхода::Внешняя{..} => {
                        диагностика!(&процедура.имя.лок, "ОШИБКА", "точкой входа в программу не может быть внешняя процедура");
                        return Err(())
                    }
                };

                let путь_к_исполняемому = файл_вывода
                    .map(|файл_вывода| PathBuf::from(файл_вывода))
                    .unwrap_or_else(||{
                        // Проверяем, что билд папка ./сборка существует
                        let папка_сборки = Path::new("./сборка");
                        if !папка_сборки.exists() {
                            fs::create_dir_all(папка_сборки).expect("Failed to create build directory");
                        }
                        let _имяфайла = &файл_ввода.file_stem().unwrap_or_default();
                        // билдим в ./сборка/_имяфайла
                        let output_path = папка_сборки.join(&_имяфайла).with_extension("");
                        output_path
                    });
                фазм::сгенерировать_исполняемый_файл(&путь_к_исполняемому, &программа.пп, &фасм, точка_входа)?;

                if пуск {
                    println!("ИНФО: запускаем «{путь_к_исполняемому}»", путь_к_исполняемому = путь_к_исполняемому.display());
                    let код_выхода = Command::new(&путь_к_исполняемому)
                        .stdout(Stdio::inherit())
                        .spawn()
                        .map_err(|ошибка| {
                            eprintln!("ОШИБКА: не получилось запустить дочерний процесс {путь_к_исполняемому}: {ошибка}",
                                      путь_к_исполняемому = путь_к_исполняемому.display());
                        })?
                        .wait()
                        .map_err(|ошибка| {
                            eprintln!("ОШИБКА: что-то пошло не так пока мы ждали завершения дочернего процесса {путь_к_исполняемому
}: {ошибка}",
                                      путь_к_исполняемому = путь_к_исполняемому.display());
                        })?;
                    #[cfg(all(unix))] {
                        use std::os::unix::process::ExitStatusExt;
                        if let Some(сигнал) = код_выхода.signal() {
                            eprintln!("ОШИБКА: дочерний процесс принудительно завершен сигналом {сигнал}");
                            return Err(())
                        }
                    }
                    match код_выхода.code() {
                        Some(0) => {}
                        Some(код) => if код != 0 {
                            eprintln!("ОШИБКА: дочерний процесс завершился с кодом {код}");
                            return Err(())
                        }
                        None => unreachable!()
                    }
                }
                Ok(())
            } else {
                eprintln!("ОШИБКА: процедура точки входа «{процедура_точки_входа}» не найдена! Пожалуйста определите её!");
                Err(())
            }
        },
    },
    Команда {
        имя: "интер",
        сигнатура: "[-отлад] <путь_к_файлу>",
        описание: "Интерпретировать Промежуточное Представление скомпилированного файла",
        запустить: |программа, mut аргы| {
            let mut режим_отладки = false;
            let mut путь_к_файлу = None;

            loop {
                match аргы.next() {
                    Some(арг) => match арг.as_str() {
                        "-отлад" => режим_отладки = true,
                        _ => {
                            if путь_к_файлу.is_some() {
                                пример(программа);
                                eprintln!("ОШИБКА: неизвестный флаг «{арг}»");
                                return Err(())
                            } else {
                                путь_к_файлу = Some(арг)
                            }
                        }
                    }
                    None => break,
                }
            }

            let путь_к_файлу = if let Some(путь_к_файлу) = путь_к_файлу {
                PathBuf::from(путь_к_файлу)
            } else {
                пример(программа);
                eprintln!("ОШИБКА: требуется файл с программой!");
                return Err(());
            };

            let содержимое: Vec<char> = прочитать_содержимое_файла(&путь_к_файлу, None)?;
            let mut лекс = Лексер::новый(&путь_к_файлу, &содержимое);
            let mut программа = Программа::default();
            программа.скомпилировать_лексемы(&mut лекс)?;
            программа.завершить_компиляцию();
            let процедура_точки_входа = "главная";
            if let Some(процедура) = программа.имена.процедуры.get(процедура_точки_входа) {
                let точка_входа = match процедура.точка_входа {
                    ТочкаВхода::Внутреняя{адрес} => адрес,
                    ТочкаВхода::Внешняя{..} => {
                        диагностика!(&процедура.имя.лок, "ОШИБКА", "точкой входа в программу не может быть внешняя процедура");
                        return Err(())
                    }
                };
                let объём_второго_стека = 1_000_000;
                let mut машина = Машина::новая(&программа.пп, объём_второго_стека);
                машина.интерпретировать(&программа.имена, точка_входа, режим_отладки)
            } else {
                eprintln!("ОШИБКА: процедура точки входа «{процедура_точки_входа}» не найдена! Пожалуйста определите её!");
                Err(())
            }
        },
    },
    Команда {
        имя: "пп",
        сигнатура: "<путь_к_файлу>",
        описание: "Напечатать Промежуточное Представление скомпилированной программы",
        запустить: |программа, mut аргы| {
            let путь_к_файлу = if let Some(путь_к_файлу) = аргы.next() {
                PathBuf::from(путь_к_файлу)
            } else {
                пример(программа);
                eprintln!("ОШИБКА: требуется файл с программой!");
                return Err(());
            };
            let содержимое: Vec<char> = прочитать_содержимое_файла(&путь_к_файлу, None)?;
            let mut лекс = Лексер::новый(&путь_к_файлу, &содержимое);
            let mut программа = Программа::default();
            программа.скомпилировать_лексемы(&mut лекс)?;
            программа.завершить_компиляцию();
            let процедура_точки_входа = "главная";
            if let Some(процедура) = программа.имена.процедуры.get(процедура_точки_входа) {
                let точка_входа = match процедура.точка_входа {
                    ТочкаВхода::Внутреняя{адрес} => адрес,
                    ТочкаВхода::Внешняя{..} => {
                        диагностика!(&процедура.имя.лок, "ОШИБКА", "точкой входа в программу не может быть внешняя процедура");
                        return Err(())
                    }
                };

                программа.пп.вывалить(точка_входа);
                Ok(())
            } else {
                eprintln!("ОШИБКА: процедура точки входа «{процедура_точки_входа}» не найдена! Пожалуйста определите её!");
                Err(())
            }
        },
    },
    Команда {
        имя: "справка",
        сигнатура: "[команда]",
        описание: "Напечатать справку по программе и командам",
        запустить: |программа, mut аргы| {
            if let Some(_имя_команды) = аргы.next() {
                todo!("СДЕЛАТЬ: справка по отдельным командам");
            } else {
                пример(программа);
                Ok(())
            }
        },
    },
];

fn пример(программа: &str) {
    eprintln!("Пример: {программа} <команда> [аргументы]");
    eprintln!("Команды:");
    let ширина_столбца_имени = КОМАНДЫ.iter().map(|команда| {
        команда.имя.chars().count()
    }).max().unwrap_or(0);
    let ширина_столбца_сигнатуры = КОМАНДЫ.iter().map(|команда| {
        команда.сигнатура.chars().count()
    }).max().unwrap_or(0);
    for Команда{имя, сигнатура, описание, ..} in КОМАНДЫ.iter() {
        // СДЕЛАТЬ: переносить длинные описания на новую строку.
        eprintln!("    {имя:ширина_столбца_имени$} {сигнатура:ширина_столбца_сигнатуры$} - {описание}");
    }
}

fn главная() -> Результат<()> {
    let mut аргы = env::args();
    let программа = аргы.next().expect("программа");

    let имя_команды = if let Some(имя_команды) = аргы.next() {
        имя_команды
    } else {
        пример(&программа);
        eprintln!("ОШИБКА: требуется команда!");
        return Err(());
    };

    if let Some(команда) = КОМАНДЫ.iter().find(|команда| имя_команды == команда.имя) {
        (команда.запустить)(&программа, аргы)
    } else {
        пример(&программа);
        eprintln!("ОШИБКА: неизвестная команда «{имя_команды}»");
        Err(())
    }
}

fn main() -> ExitCode {
    match главная() {
        Ok(()) => ExitCode::SUCCESS,
        Err(()) => ExitCode::FAILURE,
    }
}

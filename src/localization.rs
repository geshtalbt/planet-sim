//! Small runtime localisation layer (English / Russian).
//!
//! The simulator keeps its internal identifiers in English. User-facing
//! strings pass through [`tr`], which returns the Russian translation when the
//! language is set to Russian and the original English otherwise. Keeping the
//! switch global avoids threading a language argument through every Bevy
//! system while still letting the UI change language immediately.

use std::borrow::Cow;
use std::sync::atomic::{AtomicU8, Ordering};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Language {
    English,
    Russian,
}

impl Language {
    pub fn label(self) -> &'static str {
        match self {
            Self::English => "English",
            Self::Russian => "Русский",
        }
    }

    /// Best-effort system-language detection. Browsers without `LANG` fall
    /// back to English; the in-app switch is always available.
    pub fn detect() -> Self {
        if let Ok(locale) = std::env::var("LANG")
            && locale.to_ascii_lowercase().starts_with("ru")
        {
            return Self::Russian;
        }
        Self::English
    }
}

static CURRENT: AtomicU8 = AtomicU8::new(0);

pub fn set_language(language: Language) {
    CURRENT.store(
        match language {
            Language::English => 0,
            Language::Russian => 1,
        },
        Ordering::Relaxed,
    );
}

pub fn is_russian() -> bool {
    language() == Language::Russian
}

/// Pick a literal by language without allocating.
pub fn pick(english: &'static str, russian: &'static str) -> &'static str {
    if is_russian() { russian } else { english }
}

pub fn language() -> Language {
    if CURRENT.load(Ordering::Relaxed) == 1 {
        Language::Russian
    } else {
        Language::English
    }
}

/// Translate a static UI string. Unknown strings pass through unchanged.
pub fn tr(text: &'static str) -> &'static str {
    if language() == Language::Russian {
        ru(text).unwrap_or(text)
    } else {
        text
    }
}

/// Translate a runtime string (for example a recorded event title). Unknown
/// strings borrow the original, so callers can still use this for arbitrary
/// user content such as planet names.
pub fn tr_dynamic(text: &str) -> Cow<'_, str> {
    if language() == Language::Russian {
        if let Some(translated) = ru(text) {
            return Cow::Borrowed(translated);
        }
    }
    Cow::Borrowed(text)
}

/// Dynamic translation that releases the global language lock before use.
pub fn translated(text: &str) -> String {
    tr_dynamic(text).into_owned()
}

fn ru(text: &str) -> Option<&'static str> {
    Some(match text {
        "+ speed" => "+ скорость",
        "?" => "?",
        "A coupled planetary evolution laboratory" => "Связанная лаборатория эволюции планет",
        "Active eruptions" => "Активные извержения",
        "Active events" => "Активные события",
        "Add lake 5 m" => "Добавить озеро 5 м",
        "Advance" => "Продвинуть",
        "Aerosol cooling event" => "Аэрозольное похолодание",
        "Aerosol cooling" => "Аэрозольное похолодание",
        "Aerosol optical depth" => "Оптическая толщина аэрозоля",
        "Aerosol τ" => "Аэрозоль τ",
        "Albedo +0.1" => "Альбедо +0,1",
        "All values are computed from the current cell grid and global reservoirs. No statistics are generated for display alone." => {
            "Все значения вычисляются по текущей сетке ячеек и глобальным резервуарам. Статистика не создаётся только для отображения."
        }
        "Application settings" => "Настройки приложения",
        "Apply immediately" => "Применить сразу",
        "Apply water inventory" => "Применить запас воды",
        "Asteroid impact" => "Удар астероида",
        "Atmosphere and surface" => "Атмосфера и поверхность",
        "Atmosphere shell" => "Оболочка атмосферы",
        "Atmospheric CO₂" => "Атмосферный CO₂",
        "Atmospheric composition" => "Состав атмосферы",
        "Atmospheric pressure" => "Атмосферное давление",
        "Axial tilt (°)" => "Наклон оси (°)",
        "Axial tilt" => "Наклон оси",
        "Back to main menu" => "В главное меню",
        "Biome" => "Биом",
        "Black = quiet · orange = active · pale = recent lava" => {
            "Чёрный — спокойно · оранжевый — активность · бледный — свежая лава"
        }
        "Boreal forest" => "Бореальный лес",
        "Bulk properties" => "Общие свойства",
        "CH₄ (ppm)" => "CH₄ (ppm)",
        "CH₄" => "CH₄",
        "CO₂ (ppm)" => "CO₂ (ppm)",
        "CO₂ injection (ppm)" => "Впрыск CO₂ (ppm)",
        "Cancel" => "Отмена",
        "Carbon dioxide" => "Углекислый газ",
        "Carbon injection" => "Впрыск углерода",
        "Catastrophe management" => "Управление катастрофами",
        "Catastrophes" => "Катастрофы",
        "Changing total water redistributes the ocean and changes sea level; land coverage is a consequence, not an independent slider." => {
            "Изменение общего запаса воды перераспределяет воду в океане и меняет уровень моря; площадь суши — следствие, а не отдельный ползунок."
        }
        "Checkpoint now" => "Создать контрольную точку",
        "Checkpoints" => "Контрольные точки",
        "Clear persistent local forcing" => "Сбросить постоянное локальное воздействие",
        "Click the planet to inspect a region." => "Нажмите на планету, чтобы исследовать регион.",
        "Climate model notes" => "Заметки о климатической модели",
        "Climate zones / biomes" => "Климатические зоны / биомы",
        "Cloud albedo factor" => "Множитель альбедо облаков",
        "Cloud cover" => "Облачность",
        "Cloud coverage" => "Облачность",
        "Cloud veil (surface shading)" => "Облачная дымка (затенение поверхности)",
        "Cold wave" => "Волна холода",
        "Colours identify plates; pale cells lie on a boundary. Boundaries are recomputed from the simulated plate assignment." => {
            "Цвета обозначают плиты; бледные ячейки лежат на границе. Границы пересчитываются по назначению плит."
        }
        "Confirm restart" => "Подтвердить перезапуск",
        "Continents" => "Континенты",
        "Controls" => "Управление",
        "Cool −5 °C" => "Охладить −5 °C",
        "Create a fork to compare planetary histories." => {
            "Создайте ветвь, чтобы сравнить истории планет."
        }
        "Create a world, then watch its atmosphere, oceans, ice, climate and geology evolve together." => {
            "Создайте мир и наблюдайте, как атмосфера, океаны, лёд, климат и геология эволюционируют вместе."
        }
        "Create fork from active" => "Создать ветвь от активной",
        "Current" => "Текущее",
        "Planet name must not be empty" => "Имя планеты не может быть пустым",
        "Radius must be between 500 and 100,000 km" => "Радиус должен быть от 500 до 100 000 км",
        "Mass must be between 0.01 and 50 Earth masses" => {
            "Масса должна быть от 0.01 до 50 масс Земли"
        }
        "Surface gravity must be between 0.1 and 60 m/s²" => {
            "Гравитация должна быть от 0.1 до 60 м/с²"
        }
        "Stellar flux must be between 50 and 10,000 W/m²" => {
            "Поток звезды должен быть от 50 до 10 000 Вт/м²"
        }
        "Initial temperature must be between -150 and 200 °C" => {
            "Начальная температура должна быть от -150 до 200 °C"
        }
        "Surface pressure must be between 0.1 and 2,000 kPa" => {
            "Давление должно быть от 0.1 до 2 000 кПа"
        }
        "CO₂ concentration must be between 0.1 and 5,000,000 ppm" => {
            "Концентрация CO₂ должна быть от 0.1 до 5 000 000 ppm"
        }
        "Water coverage must be between 0 and 100%" => "Покрытие водой должно быть от 0 до 100%",
        "Glacier coverage must be between 0 and 100%" => {
            "Площадь ледников должна быть от 0 до 100%"
        }
        "Grid level must be between 2 and 8" => "Уровень сетки должен быть от 2 до 8",
        "Too many continents or islands for this resolution" => {
            "Слишком много континентов или островов для этого разрешения"
        }
        "Axial tilt must be between 0 and 90°" => "Наклон оси должен быть от 0 до 90°",
        "Rotation period must be between 1 and 1,000 hours" => {
            "Период вращения должен быть от 1 до 1 000 часов"
        }
        "Cloud coverage must be between 0 and 100%" => "Облачность должна быть от 0 до 100%",
        "fraction" => "доля",
        "count" => "кол-во",
        "relative" => "относительно",
        "mean" => "среднее",
        "range" => "диапазон",
        "Generating planet…" => "Генерация планеты…",
        "Planet generated from seed." => "Планета сгенерирована по seed.",
        "Checkpoint stored." => "Контрольная точка сохранена.",
        "Simulation restarted." => "Симуляция перезапущена.",
        "CO₂ ramp target updated." => "Цель ramp-изменения CO₂ обновлена.",
        "Created an independent planet fork." => "Создана независимая ветвь планеты.",
        "Water inventory rebalanced; sea level re-solved." => {
            "Запас воды перераспределён; уровень моря пересчитан."
        }
        "Target cell" => "Целевая ячейка",
        "Kinetic energy" => "Кинетическая энергия",
        "intensity" => "интенсивность",
        "CO₂" => "CO₂",
        "initial" => "начальное",
        "Tilt" => "Наклон",
        "Stellar multiplier" => "Множитель потока звезды",
        "Current effective stellar multiplier" => "Текущий эффективный множитель потока звезды",
        "Redistribute carbon between ocean / geological reservoirs" => {
            "Перераспределять углерод между океаном и геологическими резервуарами"
        }
        "Custom" => "Свой",
        "Deglaciation" => "Дегляциация",
        "Delete active fork" => "Удалить активную ветвь",
        "Delete" => "Удалить",
        "Deliberate simplifications" => "Намеренные упрощения",
        "Desert world" => "Пустынный мир",
        "Desert" => "Пустыня",
        "Discharge" => "Расход",
        "Dominant biome" => "Доминирующий биом",
        "Land" => "Суша",
        "1 minute / s" => "1 минута / с",
        "1 hour / s" => "1 час / с",
        "1 day / s" => "1 день / с",
        "1 week / s" => "1 неделя / с",
        "1 month / s" => "1 месяц / с",
        "1 year / s" => "1 год / с",
        "10 years / s" => "10 лет / с",
        "100 years / s" => "100 лет / с",
        "1,000 years / s" => "1 000 лет / с",
        "10,000 years / s" => "10 000 лет / с",
        "100,000 years / s" => "100 000 лет / с",
        "1 million years / s" => "1 млн лет / с",
        "10 million years / s" => "10 млн лет / с",
        "100 million years / s" => "100 млн лет / с",
        "1 billion years / s" => "1 млрд лет / с",
        "▶ Run" => "▶ Запуск",
        "⏸ Pause" => "⏸ Пауза",

        "Selected cell" => "Выбранная ячейка",
        "cell" => "ячейка",
        "age" => "возраст",
        "paused" => "пауза",
        "running" => "идёт",
        "checkpoint(s)" => "контрольных точек",
        "Render displacement only. Stored elevations remain real metres." => {
            "Смещение только для отображения. Сохранённые высоты остаются в реальных метрах."
        }
        "Double CO₂" => "Удвоить CO₂",
        "Drought" => "Засуха",
        "Duplicate" => "Дублировать",
        "Duration (days)" => "Длительность (дни)",
        "Duration (years)" => "Длительность (годы)",
        "Dust storm" => "Пыльная буря",
        "Dust τ" => "Пыль τ",
        "Each branch is a complete planet state, not a separate visual preset. Forks advance on the same clock and comparisons use the recorded history." => {
            "Каждая ветвь — полное состояние планеты, а не отдельный визуальный пресет. Ветви идут по одним часам, а сравнение использует записанную историю."
        }
        "Earthlike" => "Землеподобная",
        "Elevation exaggeration" => "Усиление рельефа",
        "Elevation tint plus volcanic/seismic heat overlay" => {
            "Оттенок высоты и наложение вулканической/сейсмической активности"
        }
        "Elevation" => "Высота",
        "Energy balance" => "Энергетический баланс",
        "English" => "English",
        "Eruptions inject aerosols (short cooling) and CO₂ from the geological reservoir (longer warming), and add lava relief." => {
            "Извержения выбрасывают аэрозоли (кратковременное похолодание) и CO₂ из геологического резервуара (более длительное потепление), а также создают лавовый рельеф."
        }
        "Event log" => "Журнал событий",
        "Experimental cooling forcing" => "Экспериментальное похолодание",
        "Flood" => "Наводнение",
        "Force glaciation" => "Принудительное оледенение",
        "Formation" => "Формирование",
        "Freshwater" => "Пресная вода",
        "Generate planet" => "Сгенерировать планету",
        "Generate this planet" => "Сгенерировать эту планету",
        "Geological activity" => "Геологическая активность",
        "Glaciation forcing duration (yr)" => "Длительность оледенения (лет)",
        "Glaciation forcing" => "Оледенение",
        "Glaciation forcings / Myr" => "Оледенения / млн лет",
        "Glacier coverage" => "Покрытие ледниками",
        "Global drought" => "Глобальная засуха",
        "Global mean temperature" => "Глобальная средняя температура",
        "Global temperature" => "Глобальная температура",
        "Global warming" => "Глобальное потепление",
        "Grassland" => "Луга",
        "Greenhouse g" => "Парниковый g",
        "Grid resolution" => "Разрешение сетки",
        "Hail" => "Град",
        "Heatwave" => "Волна тепла",
        "History" => "История",
        "How the model works" => "Как работает модель",
        "Humidity" => "Влажность",
        "Hurricane" => "Ураган",
        "H₂O vapour fraction" => "Доля водяного пара H₂O",
        "Ice age onset" => "Начало ледникового периода",
        "Ice and snow" => "Лёд и снег",
        "Ice coverage" => "Ледовый покров",
        "Ice thickness" => "Толщина льда",
        "Ice volume" => "Объём льда",
        "Impact angle (°)" => "Угол удара (°)",
        "Impact selected region" => "Ударить по выбранному региону",
        "Impact velocity (km/s)" => "Скорость удара (км/с)",
        "Impactor diameter (km)" => "Диаметр импактора (км)",
        "Impacts / Myr" => "Удары / млн лет",
        "Implemented" => "Реализовано",
        "Initial albedo" => "Начальное альбедо",
        "Initial glacier fraction" => "Начальная доля ледников",
        "Initial ice coverage (%)" => "Начальный ледяной покров (%)",
        "Initial mean temperature (°C)" => "Начальная средняя температура (°C)",
        "Initial ocean state" => "Начальное состояние океана",
        "Initial ocean temperature (°C)" => "Начальная температура океана (°C)",
        "Initial" => "Начальное",
        "Inject CO₂" => "Впрыснуть CO₂",
        "Intensity" => "Интенсивность",
        "Islands" => "Острова",
        "LEGEND" => "ЛЕГЕНДА",
        "Lake depth" => "Глубина озера",
        "Lake" => "Озеро",
        "Land / water" => "Суша / вода",
        "Land elevation" => "Высота суши",
        "Land fraction" => "Доля суши",
        "Land/water percentages are calibrated after terrain generation; the displayed values in the simulation are the produced values." => {
            "Доли суши и воды калибруются после генерации рельефа; в симуляции показываются фактические значения."
        }
        "Language / Язык" => "Язык / Language",
        "Large eruptions / Myr" => "Крупные извержения / млн лет",
        "Load saved planet" => "Загрузить сохранённую планету",
        "Load" => "Загрузить",
        "Local dT" => "Локальное ΔT",
        "Local pressure" => "Локальное давление",
        "Magnetic field excursion" => "Экскурс магнитного поля",
        "Magnetic field" => "Магнитное поле",
        "Magnetic reversal" => "Магнитная инверсия",
        "Major climate transition" => "Крупный климатический переход",
        "Major eruption" => "Крупное извержение",
        "Major sea-level change" => "Крупное изменение уровня моря",
        "Mars-like" => "Марсоподобная",
        "Mass (Earth masses)" => "Масса (массы Земли)",
        "Mean albedo" => "Среднее альбедо",
        "Mean land elevation (m)" => "Средняя высота суши (м)",
        "Mean precipitation" => "Средние осадки",
        "Mean temperature" => "Средняя температура",
        "Meridional ocean heat transport" => "Меридиональный перенос тепла океаном",
        "Min / max temperature" => "Мин / макс температура",
        "Name" => "Имя",
        "Natural appearance" => "Естественный вид",
        "New planet" => "Новая планета",
        "New random seed" => "Новый случайный seed",
        "No active events." => "Нет активных событий.",
        "No active simulation." => "Нет активной симуляции.",
        "No events recorded." => "События не записаны.",
        "No events yet." => "Событий пока нет.",
        "No region selected. Click the planet surface." => {
            "Регион не выбран. Нажмите на поверхность планеты."
        }
        "No saved planets in this folder." => "В этой папке нет сохранённых планет.",
        "No simulation loaded" => "Симуляция не загружена",
        "N₂ fraction" => "Доля N₂",
        "Ocean circulation change" => "Изменение циркуляции океана",
        "Ocean circulation slowdown" => "Замедление циркуляции океана",
        "Ocean depth" => "Глубина океана",
        "Ocean heat capacity (relative)" => "Теплоёмкость океана (относительная)",
        "Ocean mixed-layer heat capacity" => "Теплоёмкость перемешанного слоя океана",
        "Ocean temperature" => "Температура океана",
        "Ocean volume" => "Объём океана",
        "Ocean world" => "Океанический мир",
        "Ocean" => "Океан",
        "Open What If? comparison" => "Открыть сравнение What If?",
        "Orbital and stellar controls" => "Орбитальные и звёздные параметры",
        "O₂ / N₂" => "O₂ / N₂",
        "O₂ fraction" => "Доля O₂",
        "PLANET CREATION" => "СОЗДАНИЕ ПЛАНЕТЫ",
        "PLANET SIMULATOR" => "СИМУЛЯТОР ПЛАНЕТ",
        "PLANET STATE" => "СОСТОЯНИЕ ПЛАНЕТЫ",
        "Parameter history" => "История параметров",
        "Permanent ice sheet" => "Постоянный ледяной щит",
        "Pick another" => "Выбрать другую",
        "Planet formed" => "Планета сформирована",
        "Planet history and checkpoints" => "История планеты и контрольные точки",
        "Planet state" => "Состояние планеты",
        "Planet statistics" => "Статистика планеты",
        "Planet" => "Планета",
        "Planet-scale events" => "Планетарные события",
        "Planetary albedo" => "Планетарное альбедо",
        "Planetary physics" => "Планетарная физика",
        "Plate" => "Плита",
        "Plates" => "Плиты",
        "Polar desert" => "Полярная пустыня",
        "Precipitation" => "Осадки",
        "Preset" => "Пресет",
        "Pressure" => "Давление",
        "Quick experiments" => "Быстрые эксперименты",
        "Quit" => "Выход",
        "RECENT EVENTS" => "ПОСЛЕДНИЕ СОБЫТИЯ",
        "Radius (km)" => "Радиус (км)",
        "Rain 100 mm" => "Дождь 100 мм",
        "Rain" => "Дождь",
        "Ramp rate (ppm/yr)" => "Скорость рампы (ppm/год)",
        "Random catastrophes enabled" => "Случайные катастрофы включены",
        "Random event settings" => "Настройки случайных событий",
        "Randomise all" => "Рандомизировать всё",
        "Rapid cooling" => "Быстрое похолодание",
        "Refresh" => "Обновить",
        "Regional weather" => "Региональная погода",
        "Rename" => "Переименовать",
        "Reset to Earthlike" => "Сбросить к землеподобной",
        "Restart from initial configuration" => "Перезапуск с начальной конфигурации",
        "Restart simulation from its initial configuration" => {
            "Перезапустить симуляцию с начальной конфигурации"
        }
        "Restart" => "Перезапустить",
        "Restart-only: radius, mass, seed, grid resolution, initial temperature/ocean temperature and initial ice coverage. These regenerate the planet rather than mutate it." => {
            "Только при перезапуске: радиус, масса, seed, разрешение сетки, начальная температура/температура океана и начальный ледяной покров. Они создают планету заново, а не мутируют её."
        }
        "Restore" => "Восстановить",
        "Resume simulation" => "Продолжить симуляцию",
        "Rivers / lakes" => "Реки / озёра",
        "Rivers and flow lines" => "Реки и линии стока",
        "Rivers, lakes, water" => "Реки, озёра, вода",
        "Rotation and gravity" => "Вращение и гравитация",
        "Rotation changes the model's heat-transport/weather scaling; gravity changes the atmospheric scale height and local pressure. Neither is a full circulation solver." => {
            "Вращение меняет масштаб переноса тепла и погоды; гравитация меняет высоту однородной атмосферы и локальное давление. Это не полный расчёт циркуляции."
        }
        "Rotation period (h)" => "Период вращения (ч)",
        "Rotation period (hours)" => "Период вращения (часы)",
        "Runaway glaciation" => "Необратимое оледенение",
        "Runoff" => "Сток",
        "SELECTED REGION" => "ВЫБРАННЫЙ РЕГИОН",
        "Sandstorm" => "Песчаная буря",
        "Savanna" => "Саванна",
        "Save as" => "Сохранить как",
        "Save copy" => "Сохранить копию",
        "Save current planet" => "Сохранить текущую планету",
        "Saved planets" => "Сохранённые планеты",
        "Saves are JSON + gzip with a versioned header. Topology is rebuilt from the subdivision level on load." => {
            "Сохранения — JSON + gzip с версией формата. Топология пересоздаётся по уровню подразделения при загрузке."
        }
        "Saves" => "Сохранения",
        "Sea level" => "Уровень моря",
        "Sea-ice coverage" => "Морской лёд",
        "Sea-level anomaly" => "Аномалия уровня моря",
        "Seed" => "Сид",
        "Selected region" => "Выбранный регион",
        "Set as gradual ramp target" => "Сделать целью постепенного изменения",
        "Settings" => "Настройки",
        "Show atmosphere shell" => "Показывать оболочку атмосферы",
        "Show cloud veil" => "Показывать облачную дымку",
        "Show rivers" => "Показывать реки",
        "Simulation restarted" => "Симуляция перезапущена",
        "Simulation" => "Симуляция",
        "Snow" => "Снег",
        "Snowball" => "Снежный ком",
        "Soil water" => "Почвенная влага",
        "Statistics" => "Статистика",
        "Status" => "Статус",
        "Stellar flux (W/m²)" => "Поток звезды (Вт/м²)",
        "Stellar flux multiplier" => "Множитель потока звезды",
        "Stellar flux −20%" => "Поток звезды −20%",
        "Stellar ramp target (×)" => "Цель изменения потока (×)",
        "Step" => "Шаг",
        "Storage" => "Хранилище",
        "Store checkpoint at current time" => "Сохранить контрольную точку сейчас",
        "Super-eruption" => "Суперизвержение",
        "Surface albedo perturbation" => "Возмущение альбедо поверхности",
        "Surface albedo" => "Альбедо поверхности",
        "Surface gravity (m/s²)" => "Ускорение свободного падения (м/с²)",
        "Surface pressure (kPa)" => "Давление на поверхности (кПа)",
        "Surface pressure" => "Давление на поверхности",
        "Surface type" => "Тип поверхности",
        "Surface water coverage (%)" => "Покрытие водой (%)",
        "Surface" => "Поверхность",
        "Synchronised comparison" => "Синхронное сравнение",
        "Taiga" => "Тайга",
        "Target atmospheric CO₂ (ppm)" => "Целевой атмосферный CO₂ (ppm)",
        "Target maximum depth (m)" => "Целевая максимальная глубина (м)",
        "Target mean ocean depth (m)" => "Целевая средняя глубина океана (м)",
        "Tectonic plates" => "Тектонические плиты",
        "Tectonic reorganisation" => "Тектоническая перестройка",
        "Tectonics and events" => "Тектоника и события",
        "Temperate forest" => "Умеренный лес",
        "Temperature offsets are persistent; rainfall, lakes and volcanoes modify the stored regional state." => {
            "Температурные смещения постоянны; дождь, озёра и вулканы изменяют сохранённое состояние региона."
        }
        "Temperature" => "Температура",
        "The mesh is displaced for readability. Ocean surfaces are drawn at sea level; stored elevations are unchanged." => {
            "Сетка смещается для наглядности. Поверхность океана рисуется на уровне моря; сохранённые высоты не меняются."
        }
        "The selected screen requires an active planet." => {
            "Для выбранного экрана нужна активная планета."
        }
        "This regenerates the planet from the initial configuration and discards forks/history. Confirm?" => {
            "Планета будет создана заново, ветви и история будут удалены. Подтвердить?"
        }
        "Thunderstorm" => "Гроза",
        "Tilt +15°" => "Наклон +15°",
        "Tilt ramp target (°)" => "Цель изменения наклона (°)",
        "Timeline / checkpoints" => "Таймлайн / контрольные точки",
        "Timeline restored" => "Таймлайн восстановлен",
        "Timeline window" => "Окно времени",
        "Total water inventory (km³)" => "Общий запас воды (км³)",
        "Total water inventory" => "Общий запас воды",
        "Total water" => "Всего воды",
        "Trigger eruption" => "Запустить извержение",
        "Trigger weather event" => "Запустить погодное событие",
        "Tropical rainforest" => "Тропический лес",
        "Tropical seasonal forest" => "Тропический сезонный лес",
        "Tundra" => "Тундра",
        "Use selected region" => "Использовать выбранный регион",
        "VIEW" => "ВИД",
        "VISUALISATION LAYERS" => "СЛОИ ВИЗУАЛИЗАЦИИ",
        "Visualisation" => "Визуализация",
        "Volcanic activity" => "Вулканическая активность",
        "Volcanic eruption" => "Вулканическое извержение",
        "Volcano" => "Вулкан",
        "Warm +5 °C" => "Нагреть +5 °C",
        "Water and land" => "Вода и суша",
        "Water depth" => "Глубина воды",
        "Water inventory" => "Запас воды",
        "Weather activity multiplier" => "Множитель погодной активности",
        "What If? — planet comparison" => "Что если? — сравнение планет",
        "What If?" => "Что если?",
        "What will be simulated" => "Что будет моделироваться",
        "What-if branch created" => "Создана ветвь What If",
        "Wildfire" => "Лесной пожар",
        "activity" => "активность",
        "Ocean / lake" => "Океан / озеро",
        "Forest" => "Лес",
        "Dry land" => "Суша",
        "Ice / snow" => "Лёд / снег",
        "Lava" => "Лава",
        "Ice sheet" => "Ледяной щит",
        "Current global mean" => "Текущее глобальное среднее",
        "CO₂ is treated as well mixed; this layer is intentionally uniform." => {
            "CO₂ считается хорошо перемешанным; слой намеренно однородный."
        }
        "click Delete again to confirm" => "нажмите «Удалить» ещё раз для подтверждения",
        "greenhouse g" => "парниковый g",
        "icosphere level" => "уровень икосферы",
        "m ice" => "м льда",
        "million yr" => "млн лет",
        "rate (°/Myr)" => "скорость (°/млн лет)",
        "rate (×/yr)" => "скорость (×/год)",
        "target (ppm)" => "цель (ppm)",
        "time window" => "окно времени",
        "years" => "лет",
        "Δ ice" => "Δ льда",
        "Δ land" => "Δ суши",
        "Δ sea level" => "Δ уровня моря",
        "ΔCO₂" => "ΔCO₂",
        "ΔT °C" => "ΔT °C",
        "Русский" => "Русский",
        "← Main menu" => "← Главное меню",
        "− speed" => "− скорость",
        "☰ Menu" => "☰ Меню",
        _ => return None,
    })
}

# Supported metrics (draft for review)

Each row is one `Find Health Samples` block in the master Shortcut. **Key** is the identifier used by the API (HealthKit identifier without the `HKQuantityTypeIdentifier` / `HKCategoryTypeIdentifier` prefix). "Default" = enabled for new users. Frequency is typical for iPhone + Apple Watch.

| # | Area | HealthKit identifier | Key | Friendly name | Unit | Type | How often | Default | Dashboard card |
|---|---|---|---|---|---|---|---|---|---|
| 1 | Activity | HKQuantityTypeIdentifierStepCount | stepCount | Steps | count | quantity | every few min while moving | on | Steps, Rings |
| 2 | Activity | HKQuantityTypeIdentifierDistanceWalkingRunning | distanceWalkingRunning | Walking + running distance | km | quantity | every few min while moving | on | Steps |
| 3 | Activity | HKQuantityTypeIdentifierActiveEnergyBurned | activeEnergyBurned | Active energy (Move) | kcal | quantity | ~every minute (Watch) | on | Rings |
| 4 | Activity | HKQuantityTypeIdentifierBasalEnergyBurned | basalEnergyBurned | Resting energy | kcal | quantity | ~every few min (Watch) | off | Activity |
| 5 | Activity | HKQuantityTypeIdentifierAppleExerciseTime | appleExerciseTime | Exercise minutes | min | quantity | per minute of exercise | on | Rings |
| 6 | Activity | HKCategoryTypeIdentifierAppleStandHour | appleStandHour | Stand hours | stood / idle | category | hourly | on | Rings |
| 7 | Activity | HKQuantityTypeIdentifierAppleStandTime | appleStandTime | Stand time | min | quantity | several per hour | off | Rings |
| 8 | Activity | HKQuantityTypeIdentifierFlightsClimbed | flightsClimbed | Flights climbed | count | quantity | when climbing | on | Steps |
| 9 | Activity | HKQuantityTypeIdentifierDistanceCycling | distanceCycling | Cycling distance | km | quantity | during cycling workouts | off | Activity |
| 10 | Activity | HKQuantityTypeIdentifierDistanceSwimming | distanceSwimming | Swimming distance | m | quantity | during swim workouts | off | Activity |
| 11 | Activity | HKQuantityTypeIdentifierPhysicalEffort | physicalEffort | Physical effort | kcal/hr·kg (MET) | quantity | during activity (Watch) | off | Activity |
| 12 | Heart | HKQuantityTypeIdentifierHeartRate | heartRate | Heart rate | count/min | quantity | every few min; continuous in workouts | on | Heart |
| 13 | Heart | HKQuantityTypeIdentifierRestingHeartRate | restingHeartRate | Resting heart rate | count/min | quantity | daily | on | Heart |
| 14 | Heart | HKQuantityTypeIdentifierWalkingHeartRateAverage | walkingHeartRateAverage | Walking heart rate avg | count/min | quantity | daily | on | Heart |
| 15 | Heart | HKQuantityTypeIdentifierHeartRateVariabilitySDNN | heartRateVariabilitySDNN | HRV (SDNN) | ms | quantity | several per day/night | on | Heart, Sleep |
| 16 | Heart | HKQuantityTypeIdentifierHeartRateRecoveryOneMinute | heartRateRecoveryOneMinute | Heart rate recovery (1 min) | count/min | quantity | after workouts | off | Heart |
| 17 | Heart | HKQuantityTypeIdentifierVO2Max | vo2Max | Cardio fitness (VO₂ max) | mL/kg·min | quantity | after outdoor walks/runs, ~weekly | on | Heart |
| 18 | Heart | HKCategoryTypeIdentifierHighHeartRateEvent | highHeartRateEvent | High heart rate alert | event | category | rare | off | Heart |
| 19 | Heart | HKCategoryTypeIdentifierLowHeartRateEvent | lowHeartRateEvent | Low heart rate alert | event | category | rare | off | Heart |
| 20 | Heart | HKCategoryTypeIdentifierIrregularHeartRhythmEvent | irregularHeartRhythmEvent | Irregular rhythm notification | event | category | rare | off | Heart |
| 21 | Heart | HKQuantityTypeIdentifierAtrialFibrillationBurden | atrialFibrillationBurden | AFib history | % | quantity | weekly (if enabled on Watch) | off | Heart |
| 22 | Sleep | HKCategoryTypeIdentifierSleepAnalysis | sleepAnalysis | Sleep stages (in bed, awake, core, deep, REM) | stage enum | category | one sample per stage segment, nightly | on | Sleep score, Sleep stages |
| 23 | Sleep | HKQuantityTypeIdentifierAppleSleepingWristTemperature | appleSleepingWristTemperature | Sleeping wrist temperature | °C | quantity | nightly | on | Sleep |
| 24 | Sleep | HKCategoryTypeIdentifierSleepApneaEvent | sleepApneaEvent | Breathing disturbances notification | event | category | rare | off | Sleep |
| 25 | Respiratory | HKQuantityTypeIdentifierAppleSleepingBreathingDisturbances | appleSleepingBreathingDisturbances | Breathing disturbances | count/hr | quantity | nightly | off | Sleep |
| 26 | Respiratory | HKQuantityTypeIdentifierRespiratoryRate | respiratoryRate | Respiratory rate | count/min | quantity | during sleep, several per night | on | Sleep, Heart |
| 27 | Respiratory | HKQuantityTypeIdentifierOxygenSaturation | oxygenSaturation | Blood oxygen | % | quantity | background, a few per day/night | on | Heart |
| 28 | Body | HKQuantityTypeIdentifierBodyMass | bodyMass | Weight | kg | quantity | manual / smart scale, ~daily | on | Body comp |
| 29 | Body | HKQuantityTypeIdentifierBodyFatPercentage | bodyFatPercentage | Body fat | % | quantity | smart scale | on | Body comp |
| 30 | Body | HKQuantityTypeIdentifierLeanBodyMass | leanBodyMass | Lean body mass | kg | quantity | smart scale | on | Body comp |
| 31 | Body | HKQuantityTypeIdentifierBodyMassIndex | bodyMassIndex | BMI | count | quantity | smart scale / computed | off | Body comp |
| 32 | Body | HKQuantityTypeIdentifierHeight | height | Height | cm | quantity | rare | off | Body comp |
| 33 | Body | HKQuantityTypeIdentifierWaistCircumference | waistCircumference | Waist circumference | cm | quantity | manual | off | Body comp |
| 34 | Mobility | HKQuantityTypeIdentifierWalkingSpeed | walkingSpeed | Walking speed | km/hr | quantity | iPhone, several per day | off | Mobility |
| 35 | Mobility | HKQuantityTypeIdentifierWalkingStepLength | walkingStepLength | Step length | cm | quantity | iPhone, several per day | off | Mobility |
| 36 | Mobility | HKQuantityTypeIdentifierWalkingAsymmetryPercentage | walkingAsymmetryPercentage | Walking asymmetry | % | quantity | iPhone, several per day | off | Mobility |
| 37 | Mobility | HKQuantityTypeIdentifierWalkingDoubleSupportPercentage | walkingDoubleSupportPercentage | Double support time | % | quantity | iPhone, several per day | off | Mobility |
| 38 | Mobility | HKQuantityTypeIdentifierAppleWalkingSteadiness | appleWalkingSteadiness | Walking steadiness | % | quantity | weekly | off | Mobility |
| 39 | Mobility | HKQuantityTypeIdentifierSixMinuteWalkTestDistance | sixMinuteWalkTestDistance | Six-minute walk (estimated) | m | quantity | weekly | off | Mobility |
| 40 | Mobility | HKQuantityTypeIdentifierStairAscentSpeed | stairAscentSpeed | Stair speed: up | m/s | quantity | when climbing stairs | off | Mobility |
| 41 | Audio | HKQuantityTypeIdentifierEnvironmentalAudioExposure | environmentalAudioExposure | Environmental sound level | dBASPL | quantity | every ~30 min (Watch) | off | Hearing |
| 42 | Audio | HKQuantityTypeIdentifierHeadphoneAudioExposure | headphoneAudioExposure | Headphone audio level | dBASPL | quantity | while using headphones | off | Hearing |
| 43 | Other | HKQuantityTypeIdentifierTimeInDaylight | timeInDaylight | Time in daylight | min | quantity | throughout the day (Watch) | off | Activity |
| 44 | Other | HKCategoryTypeIdentifierMindfulSession | mindfulSession | Mindful minutes | session | category | when used | off | Mind |
| 45 | Other | HKQuantityTypeIdentifierDietaryWater | dietaryWater | Water | mL | quantity | manual / app | off | Nutrition |
| 46 | Other | HKQuantityTypeIdentifierDietaryCaffeine | dietaryCaffeine | Caffeine | mg | quantity | manual / app | off | Nutrition |

Notes
- Category values are sent as strings, e.g. `sleepAnalysis`: `inBed`, `awake`, `asleepCore`, `asleepDeep`, `asleepREM`, `asleepUnspecified`; `appleStandHour`: `stood`, `idle`.
- Workouts (`HKWorkoutType`) and State of Mind are not in this list; see open questions in [shortcut-design.md](shortcut-design.md).
- Whether each identifier appears in the Shortcuts "Find Health Samples" type picker must be verified on iOS 26 when building the master Shortcut; anything missing is import-only (export.xml).
- Apple's Sleep Score is not available (see design doc §8); healthvibe computes its own from `sleepAnalysis`.

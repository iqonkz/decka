# T32.1: the synthetic part of the wake word eval set. Writes 16 kHz WAV files made by the voices of Windows:
#   <Out>\positives  - one wake word per file, several voices, speeds and volumes
#   <Out>\negatives  - at least -Minutes of speech without the word (the text of the repository's documents), plus generated tones
# Real music and real voices: put your own WAV files into the same folders.
param(
  [Parameter(Mandatory = $true)][string]$Out,
  [int]$Minutes = 60
)
$ErrorActionPreference = 'Stop'
Add-Type -AssemblyName System.Speech
$positive = Join-Path $Out 'positives'
$negative = Join-Path $Out 'negatives'
New-Item -ItemType Directory -Force $positive, $negative | Out-Null
$format = New-Object System.Speech.AudioFormat.SpeechAudioFormatInfo(16000, [System.Speech.AudioFormat.AudioBitsPerSample]::Sixteen, [System.Speech.AudioFormat.AudioChannel]::Mono)
$speaker = New-Object System.Speech.Synthesis.SpeechSynthesizer
$voices = @($speaker.GetInstalledVoices() | Where-Object Enabled | ForEach-Object { $_.VoiceInfo })

function Save-Speech([string]$Path, [string]$Text) {
  $speaker.SetOutputToWaveFile($Path, $format)
  $speaker.Speak($Text)
  $speaker.SetOutputToNull()
}

$n = 0
foreach ($voice in $voices) {
  $speaker.SelectVoice($voice.Name)
  $words = if ($voice.Culture.Name -like 'ru*') { @('Декка', 'Эй, Декка', 'Хей, Декка') } else { @('Decka', 'Hey Decka', 'Dekka') }
  foreach ($word in $words) {
    foreach ($rate in -3, -1, 0, 2, 4) {
      foreach ($volume in 100, 45) {
        $speaker.Rate = $rate
        $speaker.Volume = $volume
        $n++
        Save-Speech (Join-Path $positive ('{0:d3}.wav' -f $n)) $word
      }
    }
  }
}

$speaker.Rate = 0
$speaker.Volume = 90
$root = Split-Path $PSScriptRoot -Parent
$russian = @($voices | Where-Object { $_.Culture.Name -like 'ru*' })
$lines = Get-ChildItem (Join-Path $root 'docs') -Recurse -Filter *.md | ForEach-Object { Get-Content $_.FullName -Encoding UTF8 } |
  Where-Object { $_.Length -gt 40 -and $_ -match '[а-яА-Я]{4}' -and $_ -notmatch '(?i)декк|decka' }
$part = 0
$bytes = 0
$block = New-Object System.Text.StringBuilder
foreach ($line in $lines) {
  [void]$block.AppendLine(($line -replace '[`*#|_\[\]]', ' '))
  if ($block.Length -lt 900) { continue }
  $voice = if ($russian.Count) { $russian[$part % $russian.Count] } else { $voices[0] }
  $speaker.SelectVoice($voice.Name)
  $file = Join-Path $negative ('speech-{0:d4}.wav' -f (++$part))
  Save-Speech $file $block.ToString()
  [void]$block.Clear()
  $bytes += (Get-Item $file).Length
  if ($bytes / 32000 / 60 -ge $Minutes) { break }
}
$speaker.Dispose()

# Ten minutes of generated "music": chords that change twice a second.
$rate = 16000
$random = New-Object System.Random 7
for ($track = 1; $track -le 10; $track++) {
  $count = $rate * 60
  $stream = [IO.File]::Create((Join-Path $negative ('tones-{0:d2}.wav' -f $track)))
  $writer = New-Object IO.BinaryWriter $stream
  $writer.Write([Text.Encoding]::ASCII.GetBytes('RIFF')); $writer.Write([int](36 + $count * 2)); $writer.Write([Text.Encoding]::ASCII.GetBytes('WAVEfmt '))
  $writer.Write([int]16); $writer.Write([int16]1); $writer.Write([int16]1); $writer.Write([int]$rate); $writer.Write([int]($rate * 2)); $writer.Write([int16]2); $writer.Write([int16]16)
  $writer.Write([Text.Encoding]::ASCII.GetBytes('data')); $writer.Write([int]($count * 2))
  $a = 220.0; $b = 330.0
  for ($i = 0; $i -lt $count; $i++) {
    if ($i % 8000 -eq 0) { $a = 110 * [Math]::Pow(2, $random.Next(0, 36) / 12.0); $b = $a * 1.5 }
    $t = $i / $rate
    $writer.Write([int16](3500 * [Math]::Sin(6.2831853 * $a * $t) + 2500 * [Math]::Sin(6.2831853 * $b * $t)))
  }
  $writer.Dispose()
}
"positives: $n files; speech: $([Math]::Round($bytes / 32000 / 60, 1)) min; tones: 10 min -> $Out"

param(
  [string]$CodexHome = "$env:USERPROFILE\.codex"
)

$ErrorActionPreference = "Stop"

$timestamp = [DateTimeOffset]::UtcNow.ToUnixTimeSeconds()
$indexPath = Join-Path $CodexHome "session_index.jsonl"
$globalPath = Join-Path $CodexHome ".codex-global-state.json"
$sessionsPath = Join-Path $CodexHome "sessions"

if (-not (Test-Path -LiteralPath $CodexHome)) {
  throw "Codex home not found: $CodexHome"
}

if (Test-Path -LiteralPath $indexPath) {
  Copy-Item -LiteralPath $indexPath -Destination (Join-Path $CodexHome "session_index.before-full-repair-$timestamp.jsonl") -Force
}

if (Test-Path -LiteralPath $globalPath) {
  Copy-Item -LiteralPath $globalPath -Destination (Join-Path $CodexHome ".codex-global-state.before-full-repair-$timestamp.json") -Force
}

$items = [ordered]@{}

if (Test-Path -LiteralPath $indexPath) {
  foreach ($line in Get-Content -LiteralPath $indexPath) {
    if ([string]::IsNullOrWhiteSpace($line)) {
      continue
    }

    try {
      $json = $line | ConvertFrom-Json
      $id = $json.id
      if (-not $id) {
        $id = $json.thread_id
      }
      if (-not $id -or $items.Contains($id)) {
        continue
      }

      $title = $json.thread_name
      if (-not $title) {
        $title = $json.title
      }
      if (-not $title) {
        $title = $json.name
      }
      if (-not $title) {
        $title = $id
      }

      $updatedAt = $json.updated_at
      if (-not $updatedAt) {
        $updatedAt = [DateTimeOffset]::UtcNow.ToString("o")
      }

      $items[$id] = [pscustomobject]@{
        id = [string]$id
        thread_name = [string]$title
        updated_at = [string]$updatedAt
      }
    } catch {
      continue
    }
  }
}

$sessionInfos = @()

if (Test-Path -LiteralPath $sessionsPath) {
  Get-ChildItem -LiteralPath $sessionsPath -Recurse -Filter *.jsonl -File | ForEach-Object {
    $id = $null
    $createdAt = $null
    $cwd = $null
    $firstUserText = $null

    foreach ($line in Get-Content -LiteralPath $_.FullName) {
      if ([string]::IsNullOrWhiteSpace($line)) {
        continue
      }

      try {
        $json = $line | ConvertFrom-Json
        if ($json.type -eq "session_meta") {
          if (-not $id) {
            $id = $json.payload.id
          }
          if (-not $createdAt) {
            $createdAt = $json.payload.timestamp
          }
          if (-not $cwd) {
            $cwd = $json.payload.cwd
          }
        } elseif ($json.payload.type -eq "message" -and $json.payload.role -eq "user" -and -not $firstUserText) {
          $texts = @()
          foreach ($content in $json.payload.content) {
            if ($content.text) {
              $texts += [string]$content.text
            }
          }
          $candidate = ($texts -join " ").Trim()
          if ($candidate -and -not $candidate.StartsWith("<environment_context>")) {
            $firstUserText = $candidate
            if ($firstUserText.Length -gt 120) {
              $firstUserText = $firstUserText.Substring(0, 120)
            }
          }
        }
      } catch {
        continue
      }
    }

    if ($id) {
      $title = if ($firstUserText) { $firstUserText } else { $id }
      $updatedAt = if ($createdAt) { $createdAt } else { [DateTimeOffset]::UtcNow.ToString("o") }

      if (-not $items.Contains($id)) {
        $items[$id] = [pscustomobject]@{
          id = [string]$id
          thread_name = [string]$title
          updated_at = [string]$updatedAt
        }
      }

      $sessionInfos += [pscustomobject]@{
        id = [string]$id
        title = [string]$title
        cwd = [string]$cwd
      }
    }
  }
}

$lines = @()
foreach ($item in $items.Values) {
  $lines += ($item | ConvertTo-Json -Compress)
}
Set-Content -LiteralPath $indexPath -Value $lines -Encoding UTF8

if (Test-Path -LiteralPath $globalPath) {
  try {
    $globalState = Get-Content -Raw -LiteralPath $globalPath | ConvertFrom-Json
  } catch {
    $globalState = [pscustomobject]@{}
  }
} else {
  $globalState = [pscustomobject]@{}
}

if (-not $globalState.'electron-persisted-atom-state') {
  $globalState | Add-Member -NotePropertyName 'electron-persisted-atom-state' -NotePropertyValue ([pscustomobject]@{}) -Force
}
$atomState = $globalState.'electron-persisted-atom-state'

if (-not $atomState.'prompt-history') {
  $atomState | Add-Member -NotePropertyName 'prompt-history' -NotePropertyValue ([pscustomobject]@{}) -Force
}
if (-not $globalState.'thread-workspace-root-hints') {
  $globalState | Add-Member -NotePropertyName 'thread-workspace-root-hints' -NotePropertyValue ([pscustomobject]@{}) -Force
}

$promptHistory = $atomState.'prompt-history'
$workspaceHints = $globalState.'thread-workspace-root-hints'

foreach ($session in $sessionInfos) {
  if (-not $promptHistory.PSObject.Properties[$session.id]) {
    $promptHistory | Add-Member -NotePropertyName $session.id -NotePropertyValue @($session.title) -Force
  }
  if ($session.cwd -and -not $workspaceHints.PSObject.Properties[$session.id]) {
    $workspaceHints | Add-Member -NotePropertyName $session.id -NotePropertyValue $session.cwd -Force
  }
}

$globalState | ConvertTo-Json -Depth 100 | Set-Content -LiteralPath $globalPath -Encoding UTF8

[pscustomobject]@{
  codexHome = $CodexHome
  indexCount = $items.Count
  sessionFileCount = $sessionInfos.Count
  backupTimestamp = $timestamp
}

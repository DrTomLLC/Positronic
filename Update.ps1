# Update.ps1 - The "Save Game" Button for Positronic
# Forces local changes to overwrite GitHub (main branch)

$ErrorActionPreference = "Stop"

Write-Host ">>> Positronic Github Sync Initiated..." -ForegroundColor Cyan

# 1. Check if git is initialized
if (-not (Test-Path ".git")) {
    Write-Host "Error: Not a git repository. Run 'git init' first." -ForegroundColor Red
    exit 1
}

# 2. Add ALL changes (staged, unstaged, and untracked)
Write-Host ">>> Staging all changes..." -ForegroundColor Yellow
git add -A

# 3. Check if there are actually changes to commit
$status = git status --porcelain
if ($status) {
    # Generate a timestamped commit message
    $timestamp = Get-Date -Format "yyyy-MM-dd HH:mm:ss"
    $commitMsg = "wip: Auto-save at $timestamp"

    Write-Host ">>> Committing with message: '$commitMsg'" -ForegroundColor Yellow
    git commit -m "$commitMsg"

    # 4. Push to main (Force update)
    Write-Host ">>> Pushing to GitHub (Force)..." -ForegroundColor Yellow
    git push origin main --force

    Write-Host ">>> SUCCESS: GitHub is now identical to this machine." -ForegroundColor Green
} else {
    Write-Host ">>> No changes detected. GitHub is already up to date." -ForegroundColor Green
}

param(
    [ValidateSet('patch', 'minor', 'major')]
    [string]$Bump = 'patch',

    [string]$Remote = 'origin',

    [switch]$DryRun
)

$ErrorActionPreference = 'Stop'

function Test-GitRepo {
    $inside = git rev-parse --is-inside-work-tree 2>$null
    if ($LASTEXITCODE -ne 0 -or $inside -ne 'true') {
        throw 'Run this script inside a git repository.'
    }
}

function Get-LatestSemVerTag {
    $tags = git tag --list --sort=-v:refname
    if ($LASTEXITCODE -ne 0) {
        throw 'Failed to read git tags.'
    }

    foreach ($tag in $tags) {
        if ($tag -match '^(v?)(\d+)\.(\d+)\.(\d+)$') {
            return [PSCustomObject]@{
                Tag    = $tag
                Prefix = $matches[1]
                Major  = [int]$matches[2]
                Minor  = [int]$matches[3]
                Patch  = [int]$matches[4]
            }
        }
    }

    return $null
}

function Get-NextTag([pscustomobject]$latest, [string]$bump) {
    $prefix = ''
    $major = 0
    $minor = 0
    $patch = 0

    if ($null -ne $latest) {
        $prefix = $latest.Prefix
        $major = $latest.Major
        $minor = $latest.Minor
        $patch = $latest.Patch
    }

    switch ($bump) {
        'major' {
            $major += 1
            $minor = 0
            $patch = 0
        }
        'minor' {
            $minor += 1
            $patch = 0
        }
        default {
            $patch += 1
        }
    }

    return ('{0}{1}.{2}.{3}' -f $prefix, $major, $minor, $patch)
}

Test-GitRepo

if (-not $DryRun) {
    $status = git status --porcelain
    if ($LASTEXITCODE -ne 0) {
        throw 'Failed to read git status.'
    }

    if (-not [string]::IsNullOrWhiteSpace($status)) {
        throw 'Working tree is not clean. Commit or stash changes before releasing.'
    }
}

$latest = Get-LatestSemVerTag
$nextTag = Get-NextTag -latest $latest -bump $Bump

if ($latest) {
    Write-Host ("Latest tag: {0}" -f $latest.Tag)
} else {
    Write-Host 'No existing semver tags found. Starting from 0.0.1 (or requested bump).'
}

Write-Host ("Next tag:   {0}" -f $nextTag)

if ($DryRun) {
    Write-Host 'Dry run enabled; no tag created or pushed.'
    exit 0
}

git tag $nextTag
if ($LASTEXITCODE -ne 0) {
    throw ("Failed to create tag {0}." -f $nextTag)
}

git push $Remote $nextTag
if ($LASTEXITCODE -ne 0) {
    throw ("Failed to push tag {0} to {1}." -f $nextTag, $Remote)
}

Write-Host ("Release tag {0} created and pushed to {1}." -f $nextTag, $Remote)

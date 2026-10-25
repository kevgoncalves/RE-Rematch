# Camera lock while kicking

What Rematch does with the camera while you charge a kick, by device, and the "lock shoot on controller" option. Read from the Blueprint dumps in `tools/rmx/out_bp/` and the assets in `tools/rmx/out/` (main checkout). Rules in our words.

## The option
- **"Lock shoot on controller"** (`BP_GameOption_LockShootOnController`, under UI/GameOptions/Camera): a gamepad option, saved as 0 (off) or 1 (on). Off by default.
- At a shot's start the game picks a shot scheme (`EShootControllerOptions`, GA_Shoot GenerateManifest):
  - **0 Keyboard**: the controller is keyboard and mouse. The option isn't read.
  - **1 Gamepad**: gamepad with the option off.
  - **2 GamepadWithLockOption**: gamepad with the option on.
- `LockCameraOption` = scheme is not 1 (GA_Shoot [8650]). So the camera locks for **keyboard and mouse always**, and for a **gamepad only with the option on**. Volleys do the same (GA_VolleyShoot OverrideComputationForKeyBoard, OverrideLockShot).
- Only shots read the option: GA_Shoot and GA_VolleyShoot. Passes and lobs (GA_Tap, GA_VolleyTap, the lob abilities) don't.

## Locked (keyboard and mouse, or gamepad with the option on)
- **The aim locks at the press.** The keyboard scheme uses `BP_MK_CameraDirectionComputation`, with `m_eLockDirectionOption = StartAbility`. The gamepad-with-lock scheme also uses that direction, with the left stick for the curve (GA_Shoot InitOverrideComputationForKeyBoard).
- **The camera turns onto the aim point and stays there.** At the start, GA_BaseShoot TryToTriggerLookAt adds a camera look-at on the crosshair's aim point:
  - it reaches the point in **0.1 s**, CubicInOut;
  - it stays locked (`m_bStayLock` true);
  - it allows **0°** of manual offset (`m_mPitchYawLimit` all 0).
  - So the mouse only curves the ball, and the right stick can't move the view.
- After the strike, once the freeze frames end, the camera looks at the shot's first bounce (GA_BaseShoot, FireTime look-at).

## Not locked (gamepad, option off)
- **No look-at at the start.** TryToTriggerLookAt only fires unlocked when the direction's lock option is StartAbility. `BP_Gamepad_CameraDirectionComputation` leaves it at the class default, which isn't StartAbility.
  - The enum's names, in the order Rematch.usmap lists them: StartAbility, ReleaseInput, SyncClientServer, FireTime.
- **The camera keeps turning while you charge, but slower.** The shot charging camera (`Shoot_CameraData`) caps the gamepad at:
  - **120°/s yaw** and **60°/s pitch**;
  - **no boost** (0; the normal camera goes from 240 to 480°/s);
  - it reads the "Aim" speed options (`m_UserSettingsTag Camera.SettingsType.Aim`).
- **From the release to the strike it can't turn at all.** `LockShoot_CameraData` has all four speeds at **0°/s**.
- **Pitch stays within the camera's own limits:** 22° down, 15° up (`m_fAboveLimitPitch`, `m_fBelowLimitPitch` in both shot cameras).
- **The aim follows the camera.** The gamepad direction is the camera's view when the shot is read, not at the press.
  - Since turning is frozen after the release, that is about where the camera pointed when you let go.
  - The launch angle comes from that camera's pitch too.
- **There is no angle limit relative to the camera at the press.** No yaw or pitch range for this exists in the shot cameras, the shot abilities or the direction computations.
  - The only limit is speed × time. A full-power shot fires by itself at 31 frames (0.52 s).
  - At 120°/s that is at most **62° of yaw** from the press (and 31° of pitch, inside the 22/15 limits).

## Passes and lobs
- Gamepad passes aim with the left stick relative to the camera, or toward the crosshair (`BP_Gamepad_Tap_FilteredInputDirectionComputation`). They use the Aim tap camera, which sets no turn speed of its own (the defaults).
- On keyboard and mouse only joystick emulation locks the camera, through the "Camera.TapLock" tag (GA_VolleyTap OverrideAimingComputationForKeyBoard). The other pass aim modes leave the mouse free.

## The SCCameraLock classes
`SCCameraLockService`, `SCCameraLockAlgoFollowingService` and `SCCameraLockFramingContainerService` (`_game_classes.md`, `_natives.tsv`) belong to the camera's lock-on-the-ball mode (WBP_CamLockBall, CamLockGamepadMapping). They have nothing to do with shots.

## What the prototype does (and where it follows Kev)
- Keyboard and mouse: shots lock, as before. The option can't change that.
- Gamepad, option on: the same as keyboard for shots.
- Gamepad, option off:
  - The camera turns at the Shoot camera's 120/60°/s while you charge a shot, and at 0°/s after the release.
  - The shot aims where the camera points at the strike, and takes its launch angle from it.
- **Kev's limit:** Kev says the camera can only move so far while kicking. The files have no such clamp, so we add one on a gamepad.
  - The limit is 62° of yaw and 31° of pitch from the camera at the press: the most the Shoot camera can turn in a full charge.
  - It applies to shots, passes and lobs, also for passes with the option on, since passes never lock.
- Keyboard-and-mouse passes and lobs keep their free mouse camera, as in the files.

## Sources
- `out_bp`: GA_Shoot.txt, GA_BaseShoot.txt, GA_VolleyShoot.txt, GA_VolleyTap.txt, GA_Tap.txt, BP_MK_CameraDirectionComputation.txt, BP_Gamepad_CameraDirectionComputation.txt, _game_classes.md, _natives.tsv.
- `out`: EShootControllerOptions.json, BP_GameOption_LockShootOnController.json, ETapAimingOptions.json, GA_BaseShoot.json (CameraLookAtParams), Shoot_CameraData.json, LockShoot_CameraData.json, AimTap_CameraData.json.
- `tools/rmx/Rematch.usmap`: the ELockDirectionOption names.
- Our notes: camera.md (turn speeds, the camera at the kick), shot-aim.md (direction lock).

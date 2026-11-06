-- SPDX-License-Identifier: AGPL-3.0-or-later
module Abi.Types
%default total

public export
abiVersion : Nat
abiVersion = 1
public export
observationSize : Nat
observationSize = 2 * 8 + 8 * 4
public export
memorySize : Nat
memorySize = 2 * 8 + 8 * 4
public export
intentSize : Nat
intentSize = 4 * 4

public export
healthOffset : Nat
healthOffset = 2 * 8
public export
contactOffset : Nat
contactOffset = 2 * 8 + 6 * 4
public export
actionOffset : Nat
actionOffset = 2 * 8 + 3 * 4
public export
targetOffset : Nat
targetOffset = 4

public export
observationLayout : Abi.Types.observationSize = 48
observationLayout = Refl
public export
memoryLayout : Abi.Types.memorySize = 48
memoryLayout = Refl
public export
intentLayout : Abi.Types.intentSize = 16
intentLayout = Refl

public export
data Seat = First | Second
public export
other : Seat -> Seat
other First = Second
other Second = First
public export
otherInvolution : (s : Seat) -> other (other s) = s
otherInvolution First = Refl
otherInvolution Second = Refl

-- Interfaces describe legal engineering states, not a parallel Rust simulation.
public export
data Phase = G1 | S | G2 | M
public export
data Transition : Phase -> Phase -> Type where
  Restriction : Transition G1 S
  Replication : Transition S G2
  Division : Transition G2 M

public export
record CommandEnvelope where
  constructor MkEnvelope
  version : Nat
  supported : version = Abi.Types.abiVersion
  seat : Seat
  sequence : Nat

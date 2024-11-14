#!/bin/bash

# update minter canister
dfx deploy minter --argument "
  (variant {
    Upgrade = record {}
  })
" --ic

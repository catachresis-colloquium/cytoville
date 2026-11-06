/* SPDX-License-Identifier: AGPL-3.0-or-later */
#include "../../generated/cytoville.h"
#include "../../generated/layout.h"
#include <assert.h>
#include <stddef.h>
_Static_assert(sizeof(CytovilleObservation)==CYTOVILLE_OBSERVATION_SIZE,"observation");
_Static_assert(sizeof(CytovilleMemory)==CYTOVILLE_MEMORY_SIZE,"memory");
_Static_assert(sizeof(CytovilleIntent)==CYTOVILLE_INTENT_SIZE,"intent");
_Static_assert(offsetof(CytovilleObservation,health)==CYTOVILLE_HEALTH_OFFSET,"health");
_Static_assert(offsetof(CytovilleObservation,contact)==CYTOVILLE_CONTACT_OFFSET,"contact");
_Static_assert(offsetof(CytovilleMemory,cached_action)==CYTOVILLE_ACTION_OFFSET,"action");
_Static_assert(offsetof(CytovilleIntent,target)==CYTOVILLE_TARGET_OFFSET,"target");
int main(void){
  CytovilleObservation observation={.tick=1,.seed=7,.health=65536,.energy=0,.threat=65536,.distance=32768,.contact=1};
  CytovilleMemory memory={0};CytovilleIntent intent={0};
  assert(cytoville_abi_version()==CYTOVILLE_ABI_VERSION);
  assert(cytoville_decide(NULL,NULL,NULL)==1);
  assert(cytoville_decide(&observation,&memory,&intent)==0);
  assert(intent.action==5);
  observation.tick=2;observation.energy=65536;
  assert(cytoville_decide(&observation,&memory,&intent)==0);
  assert(intent.action==4);
  return 0;
}

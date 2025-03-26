<!-- markdownlint-disable MD033 -->
<!-- allow inline html -->
<!-- markdownlint-disable MD028 -->
<!-- allow adjacent block elements -->

# Model Description

## Introduction

### Model Purpose

This Software Requirements Specification (SRS) describes MUSE 2.0 (**M**od**U**lar energy systems
**S**imulation **E**nvironment).
The purpose of MUSE is to provide users with a framework to simulate pathways of energy system
transition, usually in the context of climate change mitigation.

### Model Scope

<!-- markdown-link-check-disable-next-line -->
MUSE is an [Integrated Assessment Modelling](https://unfccc.int/topics/mitigation/workstreams/response-measures/modelling-tools-to-assess-the-impact-of-the-implementation-of-response-measures/integrated-assessment-models-iams-and-energy-environment-economy-e3-models)
framework that is designed to enable users to create and apply an agent-based model that simulates a
market equilibrium on a set of user-defined commodities, over a user-defined time period, for a
user-specified region or set of regions.
MUSE was developed to simulate approaches to climate change mitigation over a long time horizon
(e.g. 5-year steps to 2050 or 2100), but the framework is generalised and can therefore simulate any
market equilibrium.

## Overall Description

### Overview

MUSE 2.0 is the successor to MUSE.
The original MUSE framework is open-source software [available on GitHub](https://github.com/EnergySystemsModellingLab/MUSE_OS),
coded in Python. MUSE 2.0 is implemented following re-design of MUSE to address a range of legacy
issues that are challenging to address via upgrades to the existing MUSE framework, and to implement
the framework in the high-performance Rust language.

MUSE is classified as a recursive dynamic modelling framework in the sense that it iterates on a
single time period to find a market equilibrium, and then moves to the next time period.
Agents in MUSE have limited foresight, reacting only to information available in the current time
period.
<!-- markdown-link-check-disable-next-line -->
This is distinct from intertemporal optimisation modelling frameworks (such as [TIMES](https://iea-etsap.org/index.php/etsap-tools/model-generators/times)
and [MESSAGEix](https://docs.messageix.org/en/latest/)) which have perfect foresight over the whole
modelled time horizon.

### Model Concept

MUSE 2.0 is a bottom-up engineering-economic modelling framework that computes a price-induced
supply-demand equilibrium on a set of user-defined commodities.
It does this for each milestone time period within a user-defined time horizon.
This is a "partial equilibrium" in the sense that the framework equilibrates only the user-defined
commodities, as opposed to a whole economy.

MUSE 2.0 is data-driven in the sense that model processing and data are entirely independent, and
user-defined data is at the heart of how the model behaves. It is also "bottom-up" in nature, which
means that it requires users to characterise each individual process that produces or consumes each
commodity, along with a range of other physical, economic and agent parameters.

At a high level, the user defines:

1) The overall temporal arrangements, including the base time period,
    milestone time periods and time horizon, and within-period time
    slice lengths.

2) The service demands for each end-use (e.g. residential heating,
    steel production), for each region, and how that demand is
    distributed between the user-defined time slices within the year.
    Service demands must be given a value for the base time period and
    all milestone time periods in each region.

3) The existing capacity of each process (i.e. assets) in the base time
    period, and the year in which it was commissioned or will be
    decommissioned.

4) The techno-economic attributes (e.g. capital cost, operating costs,
    efficiency, lifetime, input and output commodities, etc) of each
    process. This must include attributes of processes existing in the
    base time period (i.e. assets) and possible future processes that
    could be adopted in future milestone time periods.

5) The agents that choose between technologies by applying search
    spaces, objectives and decision rules. Portions of demand for each
    commodity must be assigned to an agent, and the sum of these
    portions must be one.

The model takes this data, configures and self-checks, and then solves
for a system change pathway:

1) [Initialisation](#1-initialisation)
2) [Commodity Price Discovery](#2-commodity-price-discovery)
3) [Agent Investment](#3-agent-investment)
4) [Carbon Budget Solution (or CO<sub>2</sub> Price Responsiveness)](#4-carbon-budget-solution-or-co2-price-responsiveness)
5) [Recursively Solve Using Steps (2) to (5) for Each Milestone Year until End](#5-recursively-solve-using-steps-2-to-5-for-each-milestone-year-until-end)

## Framework Processing Flow

The MUSE 2.0 solution concept is as follows:

### 1. Initialisation

Read input data, performing basic temporal set up, commodity and process/asset information.
A range of input data consistency checks are performed.

### 2. Commodity Price Discovery

Dispatch Optimimisation (hereon "dispatch") is executed to determine commodity production
and consumption, with fixed asset capacities. In the first milestone year - the calibrated
base year - this step is performed before any agent investment step. In later
milestone years, it is performed after agent investment is complete to determine prices for the
following milestone year alongside production/consumption of all commodities.

#### A. Price discovery via asset dispatch optimisation using dual solution

Asset dispatch is determined via linear programming, where the objective function is the cost of
operating the system over a year, which must be minimised. The dual solution of this optimisation
are used to discover commodity price. The methodology for dispatch is described in detail in the
**Dispatch Optimisation** section of this documentation.

**Price** is discovered for each commodity, for each time slice, and for each region, using the
dual solution from the dispatch optimisation. Price is determined for each time slice by
(a) taking the dual value for the commodity balance constraint, (b) subtracting the dual value
of the capacity/availability constraint (this step is performed separately for each asset/process),
and then (c) taking the maximum value from the set of results from steps a-b.

The result is a time sliced price for each commodity in each region.

#### B. Price discovery for commodities not produced

We also calculate the prices of commodities that are not present in the dispatch
solution, but could exist in the solution for the next milestone year. These are calculated
directly from input data. This is done by calculating the marginal price of the process producing
the commodity in question with the best decision value, where decision values are calculated
using the utilisation of the next most expensive (marginal cost) asset in the dispatch stack,
adjusted for availability differences, and commodity prices from the price discovery at step 2(A).

  > **Issue 1:** There may be a better way to do this step 2(B).
  One could add all processes that could potetentially exist in the
  milestone year to the dispatch optimisation formulation, even
  those that do not have a related asset (i.e. they have no
  capacity in the milestone year). A commodity balance constraint
  could then be created for every commodity, even those that are
  not yet produced. Marginal prices could be observed from the
  dispatch optimisation result as per other commodity prices.
  This would need to be tried out to see if it works.

### 3. Agent Investment

The capacity investment of new assets required in the next milestone year are calculated as follows:

#### A. End-of-life capacity decommissioning

  We decommission assets that have reached the end of their life in the milestone year.

#### B. Agent investment

  Starting with each service demand (SVD) commodity, for each agent that produces that commodity:

##### i. Determine potential utilisation at time slice level

  For each asset/process that produces the commodity, we calculate the potential dispatch
  in each time slice. This is done by observing the amount of demand that is above the
  marginal cost of this process in the solution for the previous milestone year (i.e. demand
  that the process could competitively serve assuming nothing changes from the previous
  milestone year).

  > For example, if demand is 8 units, and asset A serves 2 units
  of that at a marginal cost of $3, and asset B serves 4 units at
  a marginal cost of $5, but process C has a marginal cost of $4,
  then the algorithm assumes that process C could serve up to 6 units
  of demand in that time slice (though also limited by its
  capacity/availability constraints in this and other time slices, etc).

  If the asset/process has lower marginal cost than any asset in a time slice, we assume
  dispatch only limited by capacity/availability constraints and demand level. If the
  process has the same marginal cost as an asset, we assume that asset is NOT displaced.
  If the process has marginal cost higher than any asset, we assume it can only serve demand
  currently unserved (i.e. due to asset decommissing at 3(A) or demadn increase), if any exists.
  If there is no asset (e.g. where demand was zero in previous milestone year, or all existing
  assets were decommissioned) then we assume dispatch only limited by process capacity/availability
  constraints and demand level.

##### ii. Calculate objective and decicion values for each asset/process

  Using the resulting set of time sliced potential utilisations, we then calculate the objective
  value/s and decision for the asset/process. For assets, the objective value is
  calculated without including capital costs (which are sunk unrecoverable costs).

  > **Issue 2:** There are some complications here, e.g. where an
  asset/process has availability constraints that interact across
  time slices, so one cannot consider one time slice at a time.
  Or when a process has both annual and time slice level
  availability constraints, which interact.

##### iii. Add new asset or confirm asset not decommissioned

  We add the best asset/process (based on objective/s and decision rule) to the capacity mix. If
  an asset, this confirms that the asset will not be decommissioned. If a process, this
  confirms the process becomes an asset. The capacity of this asset is the maximum possible,
  as limited by capacity growth, addition or aggregate limit constraints, and further limited by
  the demand level (i.e. capacity is not installed if it is more than needed to serve demand).

 > **Issue 3:** It is likely that through this iterative process we will end up with assets that
 are not performing well (as measured by objectives/decision), because their utilisation will
 change as other assets are added/confirmed. One partial solution is to continue the process -
 including recalculation of all assets' objectives/decisions at each iteration - until
 nothing changes between iterations (i.e. no new/confirmed assets) but such as approach may
 result in unintended consequences (e.g. process with nominally 2nd-best objective but low
 marginal cost being adpoted). There would likely also be convergence instabilities and complex
 interactions between assets' objectives and utilisation.

##### iv.  Repeat step 3(B)i - iii until all demand is served, then decommission any unused assets

  Once all demand is served, we decommission assets that have a utilisation of zero in all
  time slices. These assets have become stranded. This could happen when, for example,
  carbon prices are high and emitting assets become unfavorable as a result (e.g. operating them
  is too expensive and cannot compete with new technology even though the latter has capital
  cost included).

#### C. Complete agent investment

  Repeat step 3(B) for each commodity in the model, and repeat until all commodities have
  been processed.

  > **Issue 4:** The order in which this is done is important, as
  all downstream commodity demand must be known prior to agent
  investment. Service demand commodities should probably go first,
  but in most models there will be no other commodity without circular
  dependencies as described below.

  > **Issue 5:** Circularities here, e.g. power system capacity is
  required to produce H2, but also H2 can be consumed in the
  power sector so H2 capacity is needed to produce it, which
  in turn requires more power system capacity. An imperfect approach
  is to check if peak demand for each commodity has changed at the end
  of a run through all commodities, and if it has then run the capacity
  investment algorithm again for that commodity. Again, this is a
  heuristic solution that may lead to mathematical instabilities or poor
  quality solutions.

  > **Issue 6:** What about commodities that are consumed but not
  produced, or produced but not consumed? Do this capacity
  investment step only for SVD and SED commodities? And also check for
  processes that consume or produce non-balance commodities, and
  check if they can make money - invest in them if they do - requires
  specific objective of NPV.

### 4. Carbon Budget Solution (or CO<sub>2</sub> price responsiveness)

Where a CO<sub>2</sub> budget or price is specified, steps (2)-(3) are initially run with the
CO<sub>2</sub> price from the previous milestone year. After completion, we run dispatch with a
CO<sub>2</sub> budget equal to the user prescribed level (if it exists) for the new milestone year,
and record the resulting CO<sub>2</sub> price (dual solution of the CO<sub>2</sub> constraint). If
the CO<sub>2</sub> price is less than zero then re-run dispatch without the budget constraint and
set CO<sub>2</sub> price to zero. If there is no solution to the dispatch optimisation,
then the CO<sub>2</sub> budget cannot be met. In this case we re-run dispatch without the budget
constraint but with the CO<sub>2</sub> price from the previous milestone year. We warn the user
that the budget set was not met for the milestone year.

**Alternatively,** a user might specify a CO<sub>2</sub> price for
all or part of the time horizon, and no carbon budget, in which case the model runs dispatch with
the specified carbon price relating to each milestone year in steps (2)-(3) and no further
processing is needed here.

### 5. Recursively Solve using Steps (2) to (5) for each Milestone Year until End

Find commodity prices for the current milestone year as described in step (2).
The model then moves to the next milestone time period and repeats steps (3)-(4),
using these prices as inputs (i.e. assuming that prices from the previous milestone
year persist in the next milestone year). This process continues until the end of
the time horizon is reached.

  > **Issue 7:** At this point we have commodity prices for every
  time period in the simulation. The model could then perform a
  "super-loop" where the entire process above is repeated, but agents
  have some foresight of on commodity price. Super-loops will be considered
  for inclusion in a later release of MUSE.

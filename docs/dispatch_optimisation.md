# Dispatch Optimisation Formulation

Decision variables:

\\( q_{r,a,c,ts} \\), where *q* represents *c* commodity flow in region *r*, to/from asset *a*, in
time slice *ts*. Negative values are flows into the asset and positive values are flows from the
asset; *q* must be ≤0 for input flows and ≥0 for output flows. Note that *q* is a quantity flow
(e.g. energy) as opposed to an intensity (e.g. power).

where

*r* = region

*a* = asset

*c* = commodity

*ts* = time slice

Objective function:

$$
  min. \sum_{r}{\sum_{a}{\sum_{c}{\sum_{ts}}}} cost_{r,a,c,ts} * q_{r,a,c,ts}
$$

Where *cost* is a vector of cost coefficients representing the cost of
each commodity flow.

$$
  cost_{r,a,c,ts} = var\\_ opex_{r,a,pacs} + flow\\_ cost_{r,a,c} + commodity\\_ cost_{r,c,ts}
$$

*var\_opex* is the variable operating cost for a PAC. If the commodity is not a PAC, this value is
zero.

*flow\_cost* is the cost per unit flow.

*commodity\_cost* is the exogenous (user-defined) cost for a commodity. If none is defined for this
combination of parameters, this value is zero.

**NOTE:** If the commodity flow is an input (i.e. flow <0), then the value of *cost* should be
multiplied by &minus;1 so that the impact on the objective function is positive.

Constraints.

TBD – does it make sense for all assets of the same type that are in the
same region are grouped together in constraints (to reduce the number of
constraints).

## Asset-level input-output commodity balances

### Non-flexible assets

Assets where ratio between output/s and input/s is strictly proportional. Energy commodity asset
inputs and outputs are proportional to first-listed primary activity commodity at a time slice level
defined for each commodity. Input/output ratio is a fixed value.

For each *r*, *a*, *ts*, *c*:

$$ \frac{q_{r,a,c,ts}}{flow_{r,a,c}} - \frac{q_{r,a,pac1,ts}}{flow_{r,a,pac1}} = 0 $$

for all commodity flows that the process has (except *pac1*). Where *pac1* is the first listed
primary activity commodity for the asset (i.e. all input and output flows are made proportional to
*pac1* flow).

**TBD** - cases where time slice level of the commodity is seasonal or annual.

### Commodity-flexible assets

Assets where ratio of input/s to output/s can vary for selected commodities, subject to user-defined
ratios between input and output.

Energy commodity asset inputs and outputs are constrained such that total inputs to total outputs
of selected commodities is limited to user-defined ratios. Furthermore, each commodity input or
output can be limited to be within a range, relative to other commodities.

For each *r*, *a*, *c*, *ts*:

(**TBD**)

for all *c* that are flexible commodities. “in” refers to input flow commodities (i.e. with a
negative sign), and “out” refers to output flow commodities (i.e. with a positive sign).

### Asset-level capacity and availability constraints

Primary activity commodity/ies output must not exceed asset capacity or any other limit as
defined by availability factor constraint user inputs.

For the capacity limits, for each *r*, *a*, *c*, *ts*. The sum of all PACs must be less than the
assets' capacity:

$$
\sum_{pacs} \frac{q_{r,a,c,ts}}{capacity\\_ a_{a} * time\\_ slice\\_ length_{ts}} \leq 1
$$

For the availability constraints, for each *r*, *a*, *c*, *ts*:

$$
\sum_{pacs} \frac{q_{r,a,c,ts}}{capacity\\_ a_{a} * time\\_ slice\\_ length_{ts}}
\leq process.availability.value(up)_{r,a,ts}
$$

$$
\sum_{pacs} \frac{q_{r,a,c,ts}}{capacity\\_ a_{a} * time\\_ slice\\_ length_{ts}}
\geq process.availability.value(lo)_{r,a,ts}
$$

$$
\sum_{pacs} \frac{q_{r,a,c,ts}}{capacity\\_ a_{a} * time\\_ slice\\_ length_{ts}}
= process.availability.value(fx)_{r,a,ts}
$$

The sum of all PACs must be within the assets' availability bounds. Similar constraints also
limit output of PACs to respect the availability constraints at time slice, seasonal or annual
levels. With appropriate selection of *q* on the LHS to match RHS temporal granularity.

Note: Where availability is specified for a process at `daynight` time slice level, it supersedes
the capacity limit constraint (i.e. you don't need both).

### Commodity balance constraints

Commodity supply-demand balance for a whole system (or for a single region or set of regions).
For each internal commodity that requires a strict balance (supply == demand, SED), it is an
equality constraint with just “1” for each relevant commodity and RHS equals 0. Note there is also
a special case where the commodity is a service demand (e.g. Mt steel produced), where net sum of
output must be equal to the demand.

For supply-demand balance commodities. For each *r* and each *c*:

$$\sum_{a,ts} q_{r,a,c,ts} = 0$$

For a service demand, for each *c*, within a single region:

$$\sum_{a,ts} q_{r,a,c,ts} = cr\\_ net\\_ fx$$

Where *c* is a service demand commodity.

**TBD** – commodities that are consumed (so sum of *q* can be a negative value). E.g. oil reserves. \
**TBD** – trade between regions.

### Asset-level commodity flow share constraints for flexible assets

Restricts share of flow amongst a set of specified flexible commodities. Constraints can be
constructed for input side of processes or output side of processes, or both.

$$
q_{r,a,c,ts} \leq process.commodity.constraint.value(up)\_{r,a,c,ts} *
\left( \sum_{flexible\ c} q\_{r,a,c,ts} \right)
$$

$$
q_{r,a,c,ts} \geq process.commodity.constraint.value(lo)\_{r,a,c,ts} *
\left( \sum\_{flexible\ c} q_{r,a,c,ts} \right)
$$

$$
q_{r,a,c,ts} = process.commodity.constraint.value(fx)\_{r,a,c,ts} *
\left( \sum\_{flexible\ c} q\_{r,a,c,ts} \right)
$$

Could be used to define flow limits on specific commodities in a flexible process. E.g. a
refinery that is flexible and can produce gasoline, diesel or jet fuel, but for a given crude oil
input only a limited amount of jet fuel can be produced and remainder of production must be either
diesel or gasoline (for example).

### Other net and absolute commodity volume constraints

<!-- markdownlint-disable-next-line MD033 -->
Net constraint: There might be a net CO<sub>2</sub> emissions limit of zero in 2050, or even a
negative value. Constraint applied on both outputs and inputs of the commodity, sum must less then
(or equal to or more than) a user-specified value. For system-wide net commodity production
constraint, for each *c*, sum over regions, assets, time slices.

$$\sum_{r,a,ts} q_{r,a,c,ts} \leq commodity.constraint.rhs\\_ value(up)$$

$$\sum_{r,a,ts} q_{r,a,c,ts} \geq commodity.constraint.rhs\\_ value(lo)$$

$$\sum_{r,a,ts} q_{r,a,c,ts} = commodity.constraint.rhs\\_ value(fx)$$

Similar constraints can be constructed for net commodity volume over specific regions or sets of
regions.

Production or consumption constraint: Likewise similar constraints can be constructed to limit
absolute production or absolute consumption. In these cases selective choice of *q* focused on
process inputs (consumption) or process outputs (production) can be applied.
